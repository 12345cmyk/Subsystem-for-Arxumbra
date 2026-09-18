#!/usr/bin/env python3
"""Проверка согласованности ELF-лейаута (program headers <-> section headers).

BOLT-оптимизированные бинари и вышедшие из строя стриперы (см.
llvm/llvm-project#56738, #89336) могут содержать PT_LOAD-сегмент, чей
файловый диапазон [p_offset, p_offset+p_filesz) НЕ покрывает фактические
файловые смещения секций, которые сегмент содержит. Керау при загрузке
мапит такие VMA-диапазоны ошибочными байтами (или не мапит вовсе) —
бинарь падает с SIGSEGV на старте, не напечатав ни слова.

Проверки (для каждой SHF_ALLOC-секции, кроме NOBITS и пустых):
  1. VMA-диапазон секции покрывается хотя бы одним PT_LOAD
     [p_vaddr, p_vaddr+p_memsz);
  2. файловый диапазон секции [sh_offset, sh_offset+sh_size) лежит внутри
     файлового диапазона каждого такого сегмента [p_offset, p_offset+p_filesz);
  3. флаги сегмента покрывают флаги секции (SHF_EXECINSTR -> PF_X,
     SHF_WRITE -> PF_W);
  4. инвариант загрузчика: (p_vaddr - p_offset) % p_align == 0
     ("ELF load command address/offset not properly aligned").

Выход: 0 = лейаут верный, 1 = найдены нарушения, 2 = не ELF / ошибка разбора.
"""
import struct
import sys

SHT_NOBITS = 8
SHF_WRITE = 0x1
SHF_ALLOC = 0x2
SHF_EXECINSTR = 0x4
PT_LOAD = 1
PF_R, PF_W, PF_X = 4, 2, 1


def die(msg, code):
    print(f"check-elf-layout: {msg}", file=sys.stderr)
    sys.exit(code)


def main(path):
    try:
        data = open(path, "rb").read()
    except OSError as e:
        die(f"{path}: {e}", 2)
    if len(data) < 20 or data[:4] != b"\x7fELF":
        die(f"{path}: not an ELF file", 2)
    ei_class, little = data[4], data[5] == 1
    end = "<" if little else ">"
    try:
        if ei_class == 1:  # 32-bit
            e_phoff, e_shoff = struct.unpack_from(end + "II", data, 28)
            (e_ehsize, e_phentsize, e_phnum, e_shentsize, e_shnum,
             e_shstrndx) = struct.unpack_from(end + "HHHHHH", data, 38)
            sh_fmt, ph_fmt = end + "IIIIIIIIII", end + "IIIIIIII"
            sh_off_field = 16
        elif ei_class == 2:  # 64-bit
            e_phoff, e_shoff = struct.unpack_from(end + "QQ", data, 32)
            (e_ehsize, e_phentsize, e_phnum, e_shentsize, e_shnum,
             e_shstrndx) = struct.unpack_from(end + "HHHHHH", data, 52)
            sh_fmt, ph_fmt = end + "IIQQQQIIQQ", end + "IIQQQQQQ"
            sh_off_field = 24
        else:
            die(f"{path}: unsupported EI_CLASS {ei_class}", 2)

        if e_shoff == 0 or e_shnum == 0:
            print(f"check-elf-layout: {path}: no section headers; "
                  f"checked phdrs only")
            shdrs = []
        else:
            shdrs = []
            for i in range(e_shnum):
                off = e_shoff + i * e_shentsize
                vals = struct.unpack_from(sh_fmt, data, off)
                shdrs.append(dict(name=vals[0], type=vals[1], flags=vals[2],
                                  addr=vals[3], offset=vals[4], size=vals[5]))
            (shstr_off,) = struct.unpack_from(
                end + ("I" if ei_class == 1 else "Q"),
                data, e_shoff + e_shstrndx * e_shentsize + sh_off_field)

        def secname(i):
            start = shstr_off + shdrs[i]["name"]
            return data[start:data.index(b"\0", start)].decode("utf-8", "replace")

        phdrs = []
        for i in range(e_phnum):
            off = e_phoff + i * e_phentsize
            v = struct.unpack_from(ph_fmt, data, off)
            phdrs.append(dict(idx=i, type=v[0], flags=v[1], offset=v[2],
                              vaddr=v[3], paddr=v[4], filesz=v[5],
                              memsz=v[6], align=v[7]))
    except struct.error as e:
        die(f"{path}: ELF parse error: {e}", 2)

    loads = [p for p in phdrs if p["type"] == PT_LOAD]
    violations = 0

    # (4) инвариант выравнивания сегментов
    for p in phdrs:
        if p["align"] and p["align"] > 1 and (p["vaddr"] - p["offset"]) % p["align"]:
            print(f"VIOLATION: segment {p['idx']} (type 0x{p['type']:x}): "
                  f"(vaddr 0x{p['vaddr']:x} - offset 0x{p['offset']:x}) "
                  f"not aligned to p_align 0x{p['align']:x}")
            violations += 1

    # (1)(2)(3) секции внутри своих сегментов
    for i, s in enumerate(shdrs):
        if not (s["flags"] & SHF_ALLOC) or s["type"] == SHT_NOBITS or s["size"] == 0:
            continue
        name = secname(i)
        v0, v1 = s["addr"], s["addr"] + s["size"]
        f0, f1 = s["offset"], s["offset"] + s["size"]
        covering = [p for p in loads
                    if p["vaddr"] <= v0 and v1 <= p["vaddr"] + p["memsz"]]
        if not covering:
            print(f"VIOLATION: section `{name}' (vaddr 0x{v0:x}-0x{v1:x}) "
                  f"is not covered by any PT_LOAD segment")
            violations += 1
            continue
        for p in covering:
            p_f0, p_f1 = p["offset"], p["offset"] + p["filesz"]
            if not (p_f0 <= f0 and f1 <= p_f1):
                print(f"VIOLATION: section `{name}' file range 0x{f0:x}-0x{f1:x} "
                      f"is outside segment {p['idx']} file range "
                      f"0x{p_f0:x}-0x{p_f1:x} (vaddr 0x{p['vaddr']:x}, "
                      f"filesz 0x{p['filesz']:x})")
                violations += 1
            if (s["flags"] & SHF_EXECINSTR) and not (p["flags"] & PF_X):
                print(f"VIOLATION: section `{name}' is SHF_EXECINSTR but "
                      f"segment {p['idx']} has no PF_X (flags 0x{p['flags']:x})")
                violations += 1
            if (s["flags"] & SHF_WRITE) and not (p["flags"] & PF_W):
                print(f"VIOLATION: section `{name}' is SHF_WRITE but "
                      f"segment {p['idx']} has no PF_W (flags 0x{p['flags']:x})")
                violations += 1

    if violations:
        print(f"check-elf-layout: {path}: {violations} violation(s) - "
              f"LAYOUT BROKEN, бинарь не запустится")
        return 1
    print(f"check-elf-layout: {path}: layout OK ({len(loads)} LOAD segments)")
    return 0


if __name__ == "__main__":
    if len(sys.argv) != 2:
        die("usage: check-elf-layout.py <elf-file>", 2)
    sys.exit(main(sys.argv[1]))
