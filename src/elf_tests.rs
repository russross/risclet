use crate::assembler::{AssemblyOutput, assemble};
use crate::config::{Config, Mode};
use crate::elf::{
    ElfFile, ElfHeader, ElfProgramHeader, ElfSectionHeader, ElfSymbol,
};
use crate::elf_loader::{ElfInput, load_elf};

// These wire fixtures specify fields independently of the production encoder.
const HEADER: [u8; 52] = [
    0x7f, b'E', b'L', b'F', 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0xf3, 0,
    1, 0, 0, 0, 0x00, 0x10, 0, 0, 52, 0, 0, 0, 0, 1, 0, 0, 4, 0, 0, 0, 52, 0,
    32, 0, 1, 0, 40, 0, 6, 0, 4, 0,
];

#[test]
fn record_codecs_match_independent_wire_fixtures() {
    let header = ElfHeader::decode(&HEADER).unwrap();
    assert_eq!(
        header.e_ident,
        [0x7f, b'E', b'L', b'F', 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(
        (header.e_type, header.e_machine, header.e_version),
        (2, 0xf3, 1)
    );
    assert_eq!(
        (header.e_entry, header.e_phoff, header.e_shoff, header.e_flags),
        (0x1000, 52, 256, 4)
    );
    assert_eq!(
        (
            header.e_ehsize,
            header.e_phentsize,
            header.e_phnum,
            header.e_shentsize,
            header.e_shnum,
            header.e_shstrndx
        ),
        (52, 32, 1, 40, 6, 4)
    );
    assert_eq!(header.encode(), HEADER);

    let program = [
        1, 0, 0, 0, 84, 0, 0, 0, 0, 0x10, 0, 0, 0, 0x20, 0, 0, 4, 0, 0, 0, 8,
        0, 0, 0, 5, 0, 0, 0, 0, 0x10, 0, 0,
    ];
    let header = ElfProgramHeader::decode(&program).unwrap();
    assert_eq!(
        [
            header.p_type,
            header.p_offset,
            header.p_vaddr,
            header.p_paddr,
            header.p_filesz,
            header.p_memsz,
            header.p_flags,
            header.p_align
        ],
        [1, 84, 0x1000, 0x2000, 4, 8, 5, 4096]
    );
    assert_eq!(header.encode(), program);

    let section = [
        3, 0, 0, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0x10, 0, 0, 84, 0, 0, 0, 4, 0,
        0, 0, 7, 0, 0, 0, 9, 0, 0, 0, 4, 0, 0, 0, 16, 0, 0, 0,
    ];
    let header = ElfSectionHeader::decode(&section).unwrap();
    assert_eq!(
        [
            header.sh_name,
            header.sh_type,
            header.sh_flags,
            header.sh_addr,
            header.sh_offset,
            header.sh_size,
            header.sh_link,
            header.sh_info,
            header.sh_addralign,
            header.sh_entsize
        ],
        [3, 1, 6, 0x1000, 84, 4, 7, 9, 4, 16]
    );
    assert_eq!(header.encode(), section);

    let symbol = [8, 0, 0, 0, 0, 0x10, 0, 0, 4, 0, 0, 0, 0x14, 2, 0xf1, 0xff];
    let entry = ElfSymbol::decode(&symbol).unwrap();
    assert_eq!((entry.st_name, entry.st_value, entry.st_size), (8, 0x1000, 4));
    assert_eq!(
        (entry.st_info, entry.st_other, entry.st_shndx),
        (0x14, 2, 0xfff1)
    );
    assert_eq!((entry.binding(), entry.symbol_type()), (1, 4));
    assert_eq!(entry.encode(), symbol);
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

// The file deliberately has duplicate section names, extra null padding,
// renamed metadata tables, a suffix symbol reference, and NOBITS beyond EOF.
fn external_elf() -> Vec<u8> {
    let mut bytes = vec![0; 496];
    bytes[..52].copy_from_slice(&HEADER);
    for (i, value) in [1, 84, 0x1000, 0x1000, 4, 4, 5, 4096].iter().enumerate()
    {
        put_u32(&mut bytes, 52 + i * 4, *value);
    }
    bytes[84..88].copy_from_slice(&[0x13, 0, 0, 0]);
    let names = b"\0\0.text\0.text\0symbols\0names\0names-table\0tail\xff\0";
    bytes[88..88 + names.len()].copy_from_slice(names);
    let strings = b"\0prefix_entry\0file.s\0";
    bytes[160..160 + strings.len()].copy_from_slice(strings);

    // A global address symbol uses the suffix "entry" at byte offset eight.
    put_u32(&mut bytes, 208, 8);
    put_u32(&mut bytes, 212, 0x1000);
    bytes[220] = 0x10;
    bytes[222] = 1;
    put_u32(&mut bytes, 224, 14);
    bytes[236] = 0x14;
    bytes[238..240].copy_from_slice(&[0xf1, 0xff]);

    let sections = [
        [0; 10],
        [8, 1, 6, 0x1000, 84, 4, 0, 0, 4, 0],
        [14, 2, 0, 0, 192, 48, 3, 1, 4, 16],
        [22, 3, 0, 0, 160, strings.len() as u32, 0, 0, 1, 0],
        [28, 3, 0, 0, 88, names.len() as u32, 0, 0, 1, 0],
        [0, 8, 3, 0x2000, u32::MAX, 4, 0, 0, 4, 0],
    ];
    for (index, fields) in sections.iter().enumerate() {
        for (field, value) in fields.iter().enumerate() {
            put_u32(&mut bytes, 256 + index * 40 + field * 4, *value);
        }
    }
    bytes
}

#[test]
fn external_tables_preserve_offsets_and_follow_links() {
    let bytes = external_elf();
    let file = ElfFile::parse(&bytes).unwrap();
    assert_eq!(file.section_name(1).unwrap(), b".text");
    assert_eq!(file.section_name(2).unwrap(), b"symbols");
    assert_eq!(file.section_data(5).unwrap(), b"");
    let symbols = file.symbols(2).unwrap();
    assert_eq!(file.symbol_name(2, &symbols[1]).unwrap(), b"entry");

    // Loading consumes the renamed tables and excludes FILE symbols even
    // when their binding occupies the high bits of the information byte.
    let mut machine = load_elf(ElfInput::Bytes(&bytes)).unwrap();
    assert_eq!(machine.address_symbols.get(&0x1000).unwrap(), "entry");
    assert!(!machine.other_symbols.contains_key("file.s"));
    machine.reset();
    assert_eq!(machine.load(0x1000, 4).unwrap(), [0x13, 0, 0, 0]);
    assert_eq!(machine.load(0x2000, 4).unwrap(), [0; 4]);
}

#[test]
fn malformed_file_ranges_and_table_relationships_report_errors() {
    let original = external_elf();
    for (offset, value) in [
        (28, u32::MAX),
        (32, u32::MAX), // Header table ranges.
        (56, u32::MAX),
        (68, u32::MAX),            // Program payload range.
        (256 + 40 + 16, u32::MAX), // Section payload range.
        (256 + 80 + 24, 6), // String table link outside the section table.
        (256 + 80 + 24, 1), // String table link to a PROGBITS section.
        (256 + 80 + 36, 8), // Incorrect symbol entry size.
        (256 + 80 + 20, 47), // Partial symbol entry.
        (256 + 40, u32::MAX), // Section name offset.
        (208, u32::MAX),    // Symbol name offset.
    ] {
        let mut bytes = original.clone();
        put_u32(&mut bytes, offset, value);
        assert!(
            ElfFile::parse(&bytes).is_err(),
            "offset {offset}, value {value}"
        );
    }
    let mut bytes = original.clone();
    bytes[50..52].copy_from_slice(&6u16.to_le_bytes());
    assert!(ElfFile::parse(&bytes).is_err());

    // Removing a terminator or truncating a file must fail without panicking.
    let mut bytes = original.clone();
    put_u32(&mut bytes, 208, 14);
    bytes[180] = b'x';
    assert!(ElfFile::parse(&bytes).is_err());
    for length in [0, 51, 83, 255, 495] {
        assert!(ElfFile::parse(&original[..length]).is_err());
    }
}

#[test]
fn names_remain_bytes_and_machine_address_errors_are_clean() {
    let mut bytes = external_elf();
    put_u32(&mut bytes, 256 + 40, 40);
    let file = ElfFile::parse(&bytes).unwrap();
    assert_eq!(file.section_name(1).unwrap(), b"tail\xff");

    // Structurally valid file data can still violate simulator memory policy.
    for address in [0, u32::MAX - 1] {
        let mut bytes = external_elf();
        put_u32(&mut bytes, 256 + 40 + 12, address);
        assert!(ElfFile::parse(&bytes).is_ok());
        assert!(load_elf(ElfInput::Bytes(&bytes)).is_err());
    }
}

fn assembled(source: &str) -> Vec<u8> {
    let mut config = Config::for_mode(Mode::Assemble);
    match assemble(&mut config, vec![("test.s".into(), source.into())]).unwrap()
    {
        AssemblyOutput::Elf(bytes) => bytes,
        AssemblyOutput::Dumped => panic!("unexpected dump"),
    }
}

#[test]
fn assembler_layout_and_loader_agree_for_data_and_bss_combinations() {
    for (data, bss) in
        [(false, false), (true, false), (false, true), (true, true)]
    {
        let mut source = ".text\n.global _start\n_start:\nnop\n".to_string();
        if data {
            source.push_str(".data\ndata_label:\n.word 0x12345678\n");
        }
        if bss {
            source.push_str(".bss\nbss_label:\n.zero 4\n");
        }
        let bytes = assembled(&source);
        let file = ElfFile::parse(&bytes).unwrap();
        assert_eq!(file.program_headers.len(), if data || bss { 3 } else { 2 });
        let table =
            file.section_headers.iter().position(|s| s.sh_type == 2).unwrap();
        let mut machine = load_elf(ElfInput::Bytes(&bytes)).unwrap();
        machine.reset();
        assert_eq!(machine.entry_point(), file.header.e_entry);

        // Named address symbols must identify the emitted section, including
        // the BSS-only layout where BSS occupies the second non-null slot.
        for symbol in file.symbols(table).unwrap() {
            let name = file.symbol_name(table, &symbol).unwrap();
            if name == b"data_label" || name == b"bss_label" {
                let section = &file.section_headers[symbol.st_shndx as usize];
                assert_eq!(symbol.st_value, section.sh_addr);
                let expected = if name == b"data_label" {
                    [0x78, 0x56, 0x34, 0x12]
                } else {
                    [0; 4]
                };
                assert_eq!(machine.load(symbol.st_value, 4).unwrap(), expected);
            }
        }
    }
}
