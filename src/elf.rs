// ELF binary format handling for RISC-V 32-bit little-endian executables
//
// This module provides unified data structures and operations for ELF files,
// used by both the assembler (elf_builder) and simulator (elf_loader).
//
// Reference: ELF-32 Object File Format, Version 1.5 Draft 2
// https://refspecs.linuxfoundation.org/elf/elf.pdf

use crate::error::{Result, RiscletError};

// ============================================================================
// ELF Constants
// ============================================================================

// ELF Identification
pub const EI_MAG0: u8 = 0x7f;
pub const EI_MAG1: u8 = b'E';
pub const EI_MAG2: u8 = b'L';
pub const EI_MAG3: u8 = b'F';
pub const EI_CLASS: u8 = 1; // ELFCLASS32
pub const EI_DATA: u8 = 1; // ELFDATA2LSB (little endian)
pub const EI_VERSION: u8 = 1; // EV_CURRENT
pub const EI_OSABI: u8 = 0; // ELFOSABI_SYSV
pub const EI_ABIVERSION: u8 = 0;

// ELF File Types
pub const ET_EXEC: u16 = 2; // Executable file

// Machine Type
pub const EM_RISCV: u16 = 0xF3; // RISC-V

// Object File Version
pub const EV_CURRENT: u32 = 1;

// ELF Header Flags (for RISC-V)
pub const EF_RISCV_FLOAT_ABI_DOUBLE: u32 = 0x4; // Double-precision FP ABI

// Section Types
pub const SHT_NULL: u32 = 0;
pub const SHT_PROGBITS: u32 = 1;
pub const SHT_SYMTAB: u32 = 2;
pub const SHT_STRTAB: u32 = 3;
pub const SHT_NOBITS: u32 = 8;
pub const SHT_RISCV_ATTRIBUTES: u32 = 0x7000_0003;

// Section Flags
pub const SHF_WRITE: u32 = 0x1;
pub const SHF_ALLOC: u32 = 0x2;
pub const SHF_EXECINSTR: u32 = 0x4;

// Program Header Types
pub const PT_LOAD: u32 = 1;
pub const PT_RISCV_ATTRIBUTES: u32 = 0x7000_0003;

// Program Header Flags
pub const PF_X: u32 = 0x1; // Execute
pub const PF_W: u32 = 0x2; // Write
pub const PF_R: u32 = 0x4; // Read

// Symbol Binding
pub const STB_LOCAL: u8 = 0;
pub const STB_GLOBAL: u8 = 1;

// Symbol Types
pub const STT_NOTYPE: u8 = 0;
pub const STT_SECTION: u8 = 3;
pub const STT_FILE: u8 = 4;

// Special Section Indices
pub const SHN_UNDEF: u16 = 0;
pub const SHN_ABS: u16 = 0xfff1;

// ============================================================================
// ELF Data Structures
// ============================================================================

/// ELF-32 File Header
#[derive(Debug, Clone)]
pub struct ElfHeader {
    pub e_ident: [u8; 16], // ELF identification
    pub e_type: u16,       // Object file type
    pub e_machine: u16,    // Machine type
    pub e_version: u32,    // Object file version
    pub e_entry: u32,      // Entry point address
    pub e_phoff: u32,      // Program header offset
    pub e_shoff: u32,      // Section header offset
    pub e_flags: u32,      // Processor-specific flags
    pub e_ehsize: u16,     // ELF header size
    pub e_phentsize: u16,  // Program header entry size
    pub e_phnum: u16,      // Number of program headers
    pub e_shentsize: u16,  // Section header entry size
    pub e_shnum: u16,      // Number of section headers
    pub e_shstrndx: u16,   // Section name string table index
}

impl ElfHeader {
    pub const SIZE: usize = 52;

    /// Encode header to 52 bytes of little-endian binary
    pub fn encode(&self) -> [u8; Self::SIZE] {
        let mut bytes = RecordBytes::<{ Self::SIZE }>::new();
        bytes.extend_from_slice(&self.e_ident);
        bytes.extend_from_slice(&self.e_type.to_le_bytes());
        bytes.extend_from_slice(&self.e_machine.to_le_bytes());
        bytes.extend_from_slice(&self.e_version.to_le_bytes());
        bytes.extend_from_slice(&self.e_entry.to_le_bytes());
        bytes.extend_from_slice(&self.e_phoff.to_le_bytes());
        bytes.extend_from_slice(&self.e_shoff.to_le_bytes());
        bytes.extend_from_slice(&self.e_flags.to_le_bytes());
        bytes.extend_from_slice(&self.e_ehsize.to_le_bytes());
        bytes.extend_from_slice(&self.e_phentsize.to_le_bytes());
        bytes.extend_from_slice(&self.e_phnum.to_le_bytes());
        bytes.extend_from_slice(&self.e_shentsize.to_le_bytes());
        bytes.extend_from_slice(&self.e_shnum.to_le_bytes());
        bytes.extend_from_slice(&self.e_shstrndx.to_le_bytes());
        bytes.finish()
    }

    /// Decode header from 52 bytes of little-endian binary
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < Self::SIZE {
            return Err(RiscletError::elf("ELF header too short".to_string()));
        }

        let mut e_ident = [0u8; 16];
        e_ident.copy_from_slice(&data[0..16]);

        Ok(Self {
            e_ident,
            e_type: u16::from_le_bytes([data[16], data[17]]),
            e_machine: u16::from_le_bytes([data[18], data[19]]),
            e_version: u32::from_le_bytes([
                data[20], data[21], data[22], data[23],
            ]),
            e_entry: u32::from_le_bytes([
                data[24], data[25], data[26], data[27],
            ]),
            e_phoff: u32::from_le_bytes([
                data[28], data[29], data[30], data[31],
            ]),
            e_shoff: u32::from_le_bytes([
                data[32], data[33], data[34], data[35],
            ]),
            e_flags: u32::from_le_bytes([
                data[36], data[37], data[38], data[39],
            ]),
            e_ehsize: u16::from_le_bytes([data[40], data[41]]),
            e_phentsize: u16::from_le_bytes([data[42], data[43]]),
            e_phnum: u16::from_le_bytes([data[44], data[45]]),
            e_shentsize: u16::from_le_bytes([data[46], data[47]]),
            e_shnum: u16::from_le_bytes([data[48], data[49]]),
            e_shstrndx: u16::from_le_bytes([data[50], data[51]]),
        })
    }
}

/// ELF-32 Program Header
#[derive(Debug, Clone)]
pub struct ElfProgramHeader {
    pub p_type: u32,   // Segment type
    pub p_offset: u32, // Segment file offset
    pub p_vaddr: u32,  // Segment virtual address
    pub p_paddr: u32,  // Segment physical address
    pub p_filesz: u32, // Segment size in file
    pub p_memsz: u32,  // Segment size in memory
    pub p_flags: u32,  // Segment flags
    pub p_align: u32,  // Segment alignment
}

impl ElfProgramHeader {
    pub const SIZE: usize = 32;

    /// Encode program header to 32 bytes of little-endian binary
    pub fn encode(&self) -> [u8; Self::SIZE] {
        let mut bytes = RecordBytes::<{ Self::SIZE }>::new();
        bytes.extend_from_slice(&self.p_type.to_le_bytes());
        bytes.extend_from_slice(&self.p_offset.to_le_bytes());
        bytes.extend_from_slice(&self.p_vaddr.to_le_bytes());
        bytes.extend_from_slice(&self.p_paddr.to_le_bytes());
        bytes.extend_from_slice(&self.p_filesz.to_le_bytes());
        bytes.extend_from_slice(&self.p_memsz.to_le_bytes());
        bytes.extend_from_slice(&self.p_flags.to_le_bytes());
        bytes.extend_from_slice(&self.p_align.to_le_bytes());
        bytes.finish()
    }

    /// Decode program header from 32 bytes of little-endian binary
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < Self::SIZE {
            return Err(RiscletError::elf(
                "Program header too short".to_string(),
            ));
        }

        Ok(Self {
            p_type: u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
            p_offset: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
            p_vaddr: u32::from_le_bytes([data[8], data[9], data[10], data[11]]),
            p_paddr: u32::from_le_bytes([
                data[12], data[13], data[14], data[15],
            ]),
            p_filesz: u32::from_le_bytes([
                data[16], data[17], data[18], data[19],
            ]),
            p_memsz: u32::from_le_bytes([
                data[20], data[21], data[22], data[23],
            ]),
            p_flags: u32::from_le_bytes([
                data[24], data[25], data[26], data[27],
            ]),
            p_align: u32::from_le_bytes([
                data[28], data[29], data[30], data[31],
            ]),
        })
    }
}

/// ELF-32 Section Header
#[derive(Debug, Clone)]
pub struct ElfSectionHeader {
    pub sh_name: u32,      // Section name (string table index)
    pub sh_type: u32,      // Section type
    pub sh_flags: u32,     // Section flags
    pub sh_addr: u32,      // Section virtual address
    pub sh_offset: u32,    // Section file offset
    pub sh_size: u32,      // Section size in bytes
    pub sh_link: u32,      // Link to another section
    pub sh_info: u32,      // Additional section information
    pub sh_addralign: u32, // Section alignment
    pub sh_entsize: u32,   // Entry size if section holds table
}

impl ElfSectionHeader {
    pub const SIZE: usize = 40;

    /// Create a null section header
    pub fn null() -> Self {
        Self {
            sh_name: 0,
            sh_type: SHT_NULL,
            sh_flags: 0,
            sh_addr: 0,
            sh_offset: 0,
            sh_size: 0,
            sh_link: 0,
            sh_info: 0,
            sh_addralign: 0,
            sh_entsize: 0,
        }
    }

    /// Encode section header to 40 bytes of little-endian binary
    pub fn encode(&self) -> [u8; Self::SIZE] {
        let mut bytes = RecordBytes::<{ Self::SIZE }>::new();
        bytes.extend_from_slice(&self.sh_name.to_le_bytes());
        bytes.extend_from_slice(&self.sh_type.to_le_bytes());
        bytes.extend_from_slice(&self.sh_flags.to_le_bytes());
        bytes.extend_from_slice(&self.sh_addr.to_le_bytes());
        bytes.extend_from_slice(&self.sh_offset.to_le_bytes());
        bytes.extend_from_slice(&self.sh_size.to_le_bytes());
        bytes.extend_from_slice(&self.sh_link.to_le_bytes());
        bytes.extend_from_slice(&self.sh_info.to_le_bytes());
        bytes.extend_from_slice(&self.sh_addralign.to_le_bytes());
        bytes.extend_from_slice(&self.sh_entsize.to_le_bytes());
        bytes.finish()
    }

    /// Decode section header from 40 bytes of little-endian binary
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < Self::SIZE {
            return Err(RiscletError::elf(
                "Section header too short".to_string(),
            ));
        }

        Ok(Self {
            sh_name: u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
            sh_type: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
            sh_flags: u32::from_le_bytes([
                data[8], data[9], data[10], data[11],
            ]),
            sh_addr: u32::from_le_bytes([
                data[12], data[13], data[14], data[15],
            ]),
            sh_offset: u32::from_le_bytes([
                data[16], data[17], data[18], data[19],
            ]),
            sh_size: u32::from_le_bytes([
                data[20], data[21], data[22], data[23],
            ]),
            sh_link: u32::from_le_bytes([
                data[24], data[25], data[26], data[27],
            ]),
            sh_info: u32::from_le_bytes([
                data[28], data[29], data[30], data[31],
            ]),
            sh_addralign: u32::from_le_bytes([
                data[32], data[33], data[34], data[35],
            ]),
            sh_entsize: u32::from_le_bytes([
                data[36], data[37], data[38], data[39],
            ]),
        })
    }
}

/// ELF-32 Symbol Table Entry
#[derive(Debug, Clone)]
pub struct ElfSymbol {
    pub st_name: u32,  // Symbol name (string table index)
    pub st_value: u32, // Symbol value
    pub st_size: u32,  // Symbol size
    pub st_info: u8,   // Symbol type and binding
    pub st_other: u8,  // Symbol visibility
    pub st_shndx: u16, // Section index
}

impl ElfSymbol {
    pub const SIZE: usize = 16;

    pub fn binding(&self) -> u8 {
        self.st_info >> 4
    }

    pub fn symbol_type(&self) -> u8 {
        self.st_info & 0xf
    }

    /// Create undefined symbol (entry 0)
    pub fn null() -> Self {
        Self {
            st_name: 0,
            st_value: 0,
            st_size: 0,
            st_info: 0,
            st_other: 0,
            st_shndx: SHN_UNDEF,
        }
    }

    /// Create section symbol
    pub fn section(section_index: u16) -> Self {
        Self {
            st_name: 0,
            st_value: 0,
            st_size: 0,
            st_info: make_st_info(STB_LOCAL, STT_SECTION),
            st_other: 0,
            st_shndx: section_index,
        }
    }

    /// Create FILE symbol
    pub fn file(name_index: u32) -> Self {
        Self {
            st_name: name_index,
            st_value: 0,
            st_size: 0,
            st_info: make_st_info(STB_LOCAL, STT_FILE),
            st_other: 0,
            st_shndx: SHN_ABS,
        }
    }

    /// Encode symbol to 16 bytes of little-endian binary
    pub fn encode(&self) -> [u8; Self::SIZE] {
        let mut bytes = RecordBytes::<{ Self::SIZE }>::new();
        bytes.extend_from_slice(&self.st_name.to_le_bytes());
        bytes.extend_from_slice(&self.st_value.to_le_bytes());
        bytes.extend_from_slice(&self.st_size.to_le_bytes());
        bytes.extend_from_slice(&[self.st_info, self.st_other]);
        bytes.extend_from_slice(&self.st_shndx.to_le_bytes());
        bytes.finish()
    }

    /// Decode symbol from 16 bytes of little-endian binary
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < Self::SIZE {
            return Err(RiscletError::elf(
                "Symbol entry too short".to_string(),
            ));
        }

        Ok(Self {
            st_name: u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
            st_value: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
            st_size: u32::from_le_bytes([data[8], data[9], data[10], data[11]]),
            st_info: data[12],
            st_other: data[13],
            st_shndx: u16::from_le_bytes([data[14], data[15]]),
        })
    }
}

/// Helper to create st_info field from binding and type
pub fn make_st_info(bind: u8, typ: u8) -> u8 {
    (bind << 4) | (typ & 0xf)
}

// Records write fields in wire order into fixed-size storage. The cursor keeps
// encoding independent of struct layout and avoids allocating for each entry.
struct RecordBytes<const N: usize> {
    bytes: [u8; N],
    offset: usize,
}

impl<const N: usize> RecordBytes<N> {
    fn new() -> Self {
        Self { bytes: [0; N], offset: 0 }
    }

    fn extend_from_slice(&mut self, field: &[u8]) {
        let end = self.offset + field.len();
        self.bytes[self.offset..end].copy_from_slice(field);
        self.offset = end;
    }

    fn finish(self) -> [u8; N] {
        assert_eq!(self.offset, N);
        self.bytes
    }
}

/// A decoded ELF32 little-endian file with payloads borrowed from its bytes.
/// Structural checks belong here; executable and machine policy belong to users.
pub struct ElfFile<'a> {
    bytes: &'a [u8],
    pub header: ElfHeader,
    pub program_headers: Vec<ElfProgramHeader>,
    pub section_headers: Vec<ElfSectionHeader>,
}

impl<'a> ElfFile<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        let header = ElfHeader::decode(bytes)?;
        if header.e_ident[..4] != [EI_MAG0, EI_MAG1, EI_MAG2, EI_MAG3] {
            return Err(RiscletError::elf("invalid ELF magic number".into()));
        }
        if header.e_ident[4] != EI_CLASS || header.e_ident[5] != EI_DATA {
            return Err(RiscletError::elf(
                "ELF file must be 32-bit little-endian".into(),
            ));
        }
        if header.e_ident[6] != EI_VERSION || header.e_version != EV_CURRENT {
            return Err(RiscletError::elf("unsupported ELF version".into()));
        }

        // Each table is bounded once before records are decoded. Empty tables
        // need no file range, and do not require an entry-size declaration.
        if header.e_ehsize as usize != ElfHeader::SIZE {
            return Err(RiscletError::elf("unexpected ELF header size".into()));
        }
        let program_headers = decode_table(
            bytes,
            header.e_phoff,
            header.e_phnum,
            header.e_phentsize,
            ElfProgramHeader::SIZE,
            "program headers",
            ElfProgramHeader::decode,
        )?;
        let section_headers = decode_table(
            bytes,
            header.e_shoff,
            header.e_shnum,
            header.e_shentsize,
            ElfSectionHeader::SIZE,
            "section headers",
            ElfSectionHeader::decode,
        )?;
        let file = Self { bytes, header, program_headers, section_headers };

        // Validate payload ranges without allocating their contents. NOBITS
        // and NULL sections have no file payload, regardless of their size.
        for header in &file.program_headers {
            file.program_data(header)?;
        }
        for index in 0..file.section_headers.len() {
            file.section_data(index)?;
        }
        if file.header.e_shstrndx != SHN_UNDEF {
            let index = file.header.e_shstrndx as usize;
            file.string_table(index)?;
            for section in &file.section_headers {
                string_at(file.section_data(index)?, section.sh_name as usize)?;
            }
        }

        // Symbol tables declare both their record size and their associated
        // string table; neither relationship depends on section names.
        for (index, section) in file.section_headers.iter().enumerate() {
            if section.sh_type == SHT_SYMTAB {
                file.string_table(section.sh_link as usize)?;
                for symbol in file.symbols(index)? {
                    file.symbol_name(index, &symbol)?;
                }
            }
        }
        Ok(file)
    }

    pub fn program_data(&self, header: &ElfProgramHeader) -> Result<&'a [u8]> {
        file_range(
            self.bytes,
            header.p_offset,
            header.p_filesz,
            "program segment",
        )
    }

    fn section(&self, index: usize) -> Result<&ElfSectionHeader> {
        self.section_headers.get(index).ok_or_else(|| {
            RiscletError::elf(format!("section index {index} out of bounds"))
        })
    }

    pub fn section_data(&self, index: usize) -> Result<&'a [u8]> {
        let section = self.section(index)?;
        if matches!(section.sh_type, SHT_NULL | SHT_NOBITS) {
            return Ok(&[]);
        }
        file_range(self.bytes, section.sh_offset, section.sh_size, "section")
    }

    fn string_table(&self, index: usize) -> Result<&'a [u8]> {
        if self.section(index)?.sh_type != SHT_STRTAB {
            return Err(RiscletError::elf(format!(
                "section {index} is not a string table"
            )));
        }
        self.section_data(index)
    }

    pub fn section_name(&self, index: usize) -> Result<&'a [u8]> {
        let section = self.section(index)?;
        if self.header.e_shstrndx == SHN_UNDEF {
            return Ok(&[]);
        }
        string_at(
            self.string_table(self.header.e_shstrndx as usize)?,
            section.sh_name as usize,
        )
    }

    pub fn symbols(&self, index: usize) -> Result<Vec<ElfSymbol>> {
        let section = self.section(index)?;
        if section.sh_type != SHT_SYMTAB
            || section.sh_entsize as usize != ElfSymbol::SIZE
        {
            return Err(RiscletError::elf(format!(
                "invalid symbol table section {index}"
            )));
        }
        let bytes = self.section_data(index)?;
        if !bytes.len().is_multiple_of(ElfSymbol::SIZE) {
            return Err(RiscletError::elf(
                "incomplete symbol table entry".into(),
            ));
        }
        bytes
            .as_chunks::<{ ElfSymbol::SIZE }>()
            .0
            .iter()
            .map(|bytes| ElfSymbol::decode(bytes))
            .collect()
    }

    pub fn symbol_name(
        &self,
        table: usize,
        symbol: &ElfSymbol,
    ) -> Result<&'a [u8]> {
        let section = self.section(table)?;
        if section.sh_type != SHT_SYMTAB {
            return Err(RiscletError::elf(format!(
                "section {table} is not a symbol table"
            )));
        }
        string_at(
            self.string_table(section.sh_link as usize)?,
            symbol.st_name as usize,
        )
    }
}

// File ranges use subtraction so untrusted offsets cannot overflow before
// bounds checking, including on hosts with a 32-bit usize.
fn file_range<'a>(
    bytes: &'a [u8],
    offset: u32,
    size: u32,
    name: &str,
) -> Result<&'a [u8]> {
    let start = offset as usize;
    let len = size as usize;
    if start > bytes.len() || len > bytes.len() - start {
        return Err(RiscletError::elf(format!(
            "{name} out of bounds: offset {offset} size {size}"
        )));
    }
    Ok(&bytes[start..start + len])
}

fn decode_table<T>(
    bytes: &[u8],
    offset: u32,
    count: u16,
    entry_size: u16,
    expected_size: usize,
    name: &str,
    decode: fn(&[u8]) -> Result<T>,
) -> Result<Vec<T>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    if entry_size as usize != expected_size {
        return Err(RiscletError::elf(format!(
            "unexpected entry size for {name}: {entry_size}"
        )));
    }
    let data = file_range(
        bytes,
        offset,
        u32::from(count) * u32::from(entry_size),
        name,
    )?;
    data.chunks_exact(expected_size).map(decode).collect()
}

/// Preserve byte offsets, suffix references, and non-UTF-8 names verbatim.
fn string_at(bytes: &[u8], offset: usize) -> Result<&[u8]> {
    let tail = bytes.get(offset..).ok_or_else(|| {
        RiscletError::elf(format!("string offset {offset} out of bounds"))
    })?;
    let end = tail.iter().position(|&byte| byte == 0).ok_or_else(|| {
        RiscletError::elf("unterminated string in string table".into())
    })?;
    Ok(&tail[..end])
}
