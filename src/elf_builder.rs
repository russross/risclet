// ELF binary format generation for RISC-V 32-bit little-endian executables
//
// This module provides the builder for generating executable ELF binaries
// matching the layout produced by GNU as + ld.
//
// Reference: ELF-32 Object File Format, Version 1.5 Draft 2
// https://refspecs.linuxfoundation.org/elf/elf.pdf

use std::collections::HashMap;
use std::path::Path;

use crate::ast::{LineContent, LinePointer, Segment, Source};
use crate::elf::{
    EF_RISCV_FLOAT_ABI_DOUBLE, EI_ABIVERSION, EI_CLASS, EI_DATA, EI_MAG0,
    EI_MAG1, EI_MAG2, EI_MAG3, EI_OSABI, EI_VERSION, EM_RISCV, ET_EXEC,
    EV_CURRENT, ElfHeader, ElfProgramHeader, ElfSectionHeader, ElfSymbol, PF_R,
    PF_W, PF_X, PT_LOAD, PT_RISCV_ATTRIBUTES, SHF_ALLOC, SHF_EXECINSTR,
    SHF_WRITE, SHN_ABS, SHT_NOBITS, SHT_PROGBITS, SHT_RISCV_ATTRIBUTES,
    SHT_STRTAB, SHT_SYMTAB, STB_GLOBAL, STB_LOCAL, STT_NOTYPE, make_st_info,
};
use crate::error::{Result, RiscletError};
use crate::expressions::{EvaluatedValue, SymbolValues};
use crate::layout::{Layout, LineLayout};
use crate::symbols::{SymbolDefinition, SymbolLinks, is_internal_symbol};

// ============================================================================
// ELF Builder
// ============================================================================

/// Section order and header reservation for the assembler's executable format.
/// Relaxation and emission share this policy so addresses use the same counts.
pub struct ExecutableLayout {
    text: u16,
    data: Option<u16>,
    bss: Option<u16>,
    strtab: u16,
    shstrtab: u16,
}

impl ExecutableLayout {
    pub fn new(has_data: bool, has_bss: bool) -> Self {
        let data = has_data.then_some(2);
        let bss = has_bss.then_some(2 + u16::from(has_data));
        let attributes = 2 + u16::from(has_data) + u16::from(has_bss);
        Self {
            text: 1,
            data,
            bss,
            strtab: attributes + 2,
            shstrtab: attributes + 3,
        }
    }

    fn program_header_count(&self) -> usize {
        2 + usize::from(self.data.is_some() || self.bss.is_some())
    }

    pub fn header_size(&self) -> u32 {
        (ElfHeader::SIZE + self.program_header_count() * ElfProgramHeader::SIZE)
            as u32
    }
}

pub struct ElfBuilder<'a> {
    header: ElfHeader,
    program_headers: Vec<ElfProgramHeader>,
    section_headers: Vec<ElfSectionHeader>,
    section_names: StringTable,
    symbol_table: Vec<ElfSymbol>,
    symbol_names: StringTable,
    text_data: Vec<u8>,
    data_data: Vec<u8>,
    riscv_attributes: Vec<u8>,
    executable_layout: ExecutableLayout,
    layout: &'a Layout,
}

impl<'a> ElfBuilder<'a> {
    pub fn new(
        layout: &'a Layout,
        text_data: Vec<u8>,
        data_data: Vec<u8>,
    ) -> Self {
        Self {
            header: executable_header(),
            program_headers: Vec::new(),
            section_headers: Vec::new(),
            section_names: StringTable::new(),
            symbol_table: Vec::new(),
            symbol_names: StringTable::new(),
            text_data,
            data_data,
            riscv_attributes: generate_riscv_attributes(),
            executable_layout: ExecutableLayout::new(
                layout.data_size > 0,
                layout.bss_size > 0,
            ),
            layout,
        }
    }

    /// Add a symbol to the symbol table
    fn add_symbol(&mut self, symbol: ElfSymbol) {
        self.symbol_table.push(symbol);
    }

    /// Build the complete ELF file
    pub fn build(mut self, entry_point: u32) -> Result<Vec<u8>> {
        self.header.e_entry = entry_point;

        let mut output = vec![0; ElfHeader::SIZE];

        // Pre-populate section name string table (needed before building section headers)
        // This ensures all section names are in the string table before we reference them
        self.section_names.add(".text");
        if self.executable_layout.data.is_some() {
            self.section_names.add(".data");
        }
        if self.executable_layout.bss.is_some() {
            self.section_names.add(".bss");
        }
        self.section_names.add(".riscv.attributes");
        self.section_names.add(".symtab");
        self.section_names.add(".strtab");
        self.section_names.add(".shstrtab");

        // Reserve the same header footprint used to compute source addresses.
        let phoff = output.len() as u32;
        let ph_size = (self.executable_layout.program_header_count()
            * ElfProgramHeader::SIZE) as u32;
        let actual_header_size = phoff + ph_size;

        // Validation check: ensure the estimated header size matches the actual size.
        // A mismatch indicates a bug in the program header count estimation.
        if self.layout.header_size != actual_header_size {
            return Err(RiscletError::internal(format!(
                "ELF header size mismatch: estimated {} but actual is {} \
                 (the number of program headers was likely estimated incorrectly)",
                self.layout.header_size, actual_header_size
            )));
        }
        output.resize(output.len() + ph_size as usize, 0);

        // --- Section Layout ---
        let page_size = 0x1000;

        // .text section starts right after the program headers
        let text_offset = output.len() as u32;
        output.extend_from_slice(&self.text_data);

        // .data section is page-aligned in the file to support mmap.
        // Pad the file with zeros to align the data offset.
        let data_offset = if self.executable_layout.data.is_some()
            || self.executable_layout.bss.is_some()
        {
            let current_len = output.len() as u32;
            let padding = (page_size - (current_len % page_size)) % page_size;
            output.resize(output.len() + padding as usize, 0);
            Some(output.len() as u32)
        } else {
            None
        };

        if let Some(_offset) = data_offset {
            output.extend_from_slice(&self.data_data);
        }

        // .riscv.attributes section (not loaded into memory)
        let riscv_attrs_offset = output.len() as u32;
        output.extend_from_slice(&self.riscv_attributes);

        // Build symbol table
        if self.symbol_table.is_empty() {
            self.symbol_table.push(ElfSymbol::null());
        }

        // Symbol table section
        let symtab_offset = output.len() as u32;
        for sym in &self.symbol_table {
            output.extend_from_slice(&sym.encode());
        }

        // String table section (.strtab)
        let strtab_offset = output.len() as u32;
        output.extend_from_slice(self.symbol_names.data());

        // Section name string table (.shstrtab)
        let shstrtab_offset = output.len() as u32;
        output.extend_from_slice(self.section_names.data());

        // Build section headers
        self.build_section_headers(
            text_offset,
            data_offset,
            riscv_attrs_offset,
            symtab_offset,
            strtab_offset,
            shstrtab_offset,
        )?;

        // Write section headers
        let shoff = output.len() as u32;
        for sh in &self.section_headers {
            output.extend_from_slice(&sh.encode());
        }

        // Program headers are constructed only after their file offsets exist.
        self.build_program_headers(
            text_offset,
            data_offset,
            riscv_attrs_offset,
        );

        // Update ELF header
        self.header.e_phoff = phoff;
        self.header.e_phnum = self.program_headers.len() as u16;
        self.header.e_shoff = shoff;
        self.header.e_shnum = self.section_headers.len() as u16;
        self.header.e_shstrndx = self.executable_layout.shstrtab;

        // Write ELF header at the beginning
        output[..ElfHeader::SIZE].copy_from_slice(&self.header.encode());

        // Write program headers at their reserved location
        for (slot, header) in output[phoff as usize..(phoff + ph_size) as usize]
            .as_chunks_mut::<{ ElfProgramHeader::SIZE }>()
            .0
            .iter_mut()
            .zip(&self.program_headers)
        {
            slot.copy_from_slice(&header.encode());
        }

        Ok(output)
    }

    fn build_program_headers(
        &mut self,
        text_offset: u32,
        data_offset: Option<u32>,
        attributes_offset: u32,
    ) {
        // RISCV_ATTRIBUTES segment (non-allocating)
        self.program_headers.push(ElfProgramHeader {
            p_type: PT_RISCV_ATTRIBUTES,
            p_offset: attributes_offset,
            p_vaddr: 0,
            p_paddr: 0,
            p_filesz: self.riscv_attributes.len() as u32,
            p_memsz: 0,
            p_flags: PF_R,
            p_align: 1,
        });

        // The text mapping includes the file headers before the first instruction.
        let text_filesz = text_offset + self.text_data.len() as u32;
        let base_vaddr = self.layout.text_start.saturating_sub(text_offset);
        self.program_headers.push(ElfProgramHeader {
            p_type: PT_LOAD,
            p_offset: 0,
            p_vaddr: base_vaddr,
            p_paddr: base_vaddr,
            p_filesz: text_filesz,
            p_memsz: text_filesz,
            p_flags: PF_R | PF_X,
            p_align: 0x1000,
        });

        // LOAD segment for .data + .bss (if present)
        if let Some(offset) = data_offset {
            let data_filesz = self.data_data.len() as u32;
            let data_memsz = data_filesz + self.layout.bss_size;

            self.program_headers.push(ElfProgramHeader {
                p_type: PT_LOAD,
                p_offset: offset,
                p_vaddr: self.layout.data_start,
                p_paddr: self.layout.data_start,
                p_filesz: data_filesz,
                p_memsz: data_memsz,
                p_flags: PF_R | PF_W,
                p_align: 0x1000,
            });
        }
    }

    fn build_section_headers(
        &mut self,
        text_offset: u32,
        data_offset: Option<u32>,
        riscv_attrs_offset: u32,
        symtab_offset: u32,
        strtab_offset: u32,
        shstrtab_offset: u32,
    ) -> Result<()> {
        // Section 0: NULL
        self.section_headers.push(ElfSectionHeader::null());

        // Section 1: .text
        self.section_headers.push(ElfSectionHeader {
            sh_name: self.section_names.add(".text"),
            sh_type: SHT_PROGBITS,
            sh_flags: SHF_ALLOC | SHF_EXECINSTR,
            sh_addr: self.layout.text_start,
            sh_offset: text_offset,
            sh_size: self.text_data.len() as u32,
            sh_link: 0,
            sh_info: 0,
            sh_addralign: 4,
            sh_entsize: 0,
        });

        // The optional data section follows text.
        if self.executable_layout.data.is_some() {
            self.section_headers.push(ElfSectionHeader {
                sh_name: self.section_names.add(".data"),
                sh_type: SHT_PROGBITS,
                sh_flags: SHF_WRITE | SHF_ALLOC,
                sh_addr: self.layout.data_start,
                sh_offset: data_offset.ok_or_else(|| {
                    RiscletError::internal(
                        "data offset should be set when data section exists"
                            .to_string(),
                    )
                })?,
                sh_size: self.data_data.len() as u32,
                sh_link: 0,
                sh_info: 0,
                sh_addralign: 1,
                sh_entsize: 0,
            });
        }

        // BSS follows the optional data section and occupies no file bytes.
        if self.executable_layout.bss.is_some() {
            self.section_headers.push(ElfSectionHeader {
                sh_name: self.section_names.add(".bss"),
                sh_type: SHT_NOBITS,
                sh_flags: SHF_WRITE | SHF_ALLOC,
                sh_addr: self.layout.bss_start,
                sh_offset: data_offset
                    .unwrap_or(text_offset + self.text_data.len() as u32),
                sh_size: self.layout.bss_size,
                sh_link: 0,
                sh_info: 0,
                sh_addralign: 1,
                sh_entsize: 0,
            });
        }

        // Section: .riscv.attributes
        self.section_headers.push(ElfSectionHeader {
            sh_name: self.section_names.add(".riscv.attributes"),
            sh_type: SHT_RISCV_ATTRIBUTES,
            sh_flags: 0, // Not allocated
            sh_addr: 0,
            sh_offset: riscv_attrs_offset,
            sh_size: self.riscv_attributes.len() as u32,
            sh_link: 0,
            sh_info: 0,
            sh_addralign: 1,
            sh_entsize: 0,
        });

        // Section: .symtab
        let first_global =
            self.symbol_table
                .iter()
                .position(|sym| sym.binding() == STB_GLOBAL)
                .unwrap_or(self.symbol_table.len()) as u32;

        self.section_headers.push(ElfSectionHeader {
            sh_name: self.section_names.add(".symtab"),
            sh_type: SHT_SYMTAB,
            sh_flags: 0,
            sh_addr: 0,
            sh_offset: symtab_offset,
            sh_size: (self.symbol_table.len() * ElfSymbol::SIZE) as u32,
            sh_link: u32::from(self.executable_layout.strtab),
            sh_info: first_global, // Index of first global symbol
            sh_addralign: 8,
            sh_entsize: ElfSymbol::SIZE as u32,
        });

        // Section: .strtab
        self.section_headers.push(ElfSectionHeader {
            sh_name: self.section_names.add(".strtab"),
            sh_type: SHT_STRTAB,
            sh_flags: 0,
            sh_addr: 0,
            sh_offset: strtab_offset,
            sh_size: self.symbol_names.len() as u32,
            sh_link: 0,
            sh_info: 0,
            sh_addralign: 1,
            sh_entsize: 0,
        });

        // Section: .shstrtab
        self.section_headers.push(ElfSectionHeader {
            sh_name: self.section_names.add(".shstrtab"),
            sh_type: SHT_STRTAB,
            sh_flags: 0,
            sh_addr: 0,
            sh_offset: shstrtab_offset,
            sh_size: self.section_names.len() as u32,
            sh_link: 0,
            sh_info: 0,
            sh_addralign: 1,
            sh_entsize: 0,
        });

        Ok(())
    }

    /// Build symbol table from Source and SymbolLinks
    ///
    /// Symbol ordering matches GNU toolchain:
    /// 1. Null symbol (entry 0)
    /// 2. Section symbols (.text, .data, .bss if present)
    /// 3. For each source file:
    ///    a. FILE symbol
    ///    b. Special $xrv32i2p1_m2p0_a2p1_c2p0 marker symbol
    ///    c. Local labels from that file
    /// 4. Global symbols (including linker-provided symbols)
    pub fn build_symbol_table(
        &mut self,
        source: &Source,
        symbol_links: &SymbolLinks,
        symbol_values: &SymbolValues,
    ) -> Result<()> {
        let text_start = self.layout.text_start;
        let data_start = self.layout.data_start;
        let bss_start = self.layout.bss_start;

        // Entry 0: Null symbol
        self.add_symbol(ElfSymbol::null());

        // Section symbols
        let text_section_index = self.executable_layout.text;
        self.add_symbol(ElfSymbol::section(text_section_index));

        let data_section_index = self.executable_layout.data;
        if let Some(index) = data_section_index {
            self.add_symbol(ElfSymbol::section(index));
        }

        let bss_section_index = self.executable_layout.bss;
        if let Some(index) = bss_section_index {
            self.add_symbol(ElfSymbol::section(index));
        }

        // For each source file, add FILE symbol and local labels
        // Skip the builtin file (last file) as it's not a real source file
        let skip = source.files.len() - 1;
        for (file_index, source_file) in source.files.iter().enumerate() {
            if file_index == skip {
                continue;
            }

            // FILE symbol (basename of source file)
            let file_name = Path::new(&source_file.file)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&source_file.file);
            let file_name_idx = self.symbol_names.add(file_name);
            self.add_symbol(ElfSymbol::file(file_name_idx));

            // Add special $xrv32i2p1_m2p0_a2p1_c2p0 marker symbol
            // This marks the start of code from this file
            let marker_name =
                self.symbol_names.add("$xrv32i2p1_m2p0_a2p1_c2p0");

            // Find the first .text line in this file to use as the marker address
            let mut marker_addr = text_start;
            for (line_index, _line) in source_file.lines.iter().enumerate() {
                let pointer = LinePointer { file_index, line_index };
                if let &LineLayout { offset, segment: Segment::Text, .. } =
                    self.layout.get(pointer)
                {
                    marker_addr = text_start + offset;
                    break;
                }
            }

            self.add_symbol(ElfSymbol {
                st_name: marker_name,
                st_value: marker_addr,
                st_size: 0,
                st_info: make_st_info(STB_LOCAL, STT_NOTYPE),
                st_other: 0,
                st_shndx: text_section_index,
            });

            // Add local labels (non-global labels and non-.equ symbols)
            for (line_index, line) in source_file.lines.iter().enumerate() {
                if let LineContent::Label(name) = &line.content {
                    // Skip if this label is declared global
                    let is_global =
                        symbol_links.global_symbols.iter().any(|g| {
                            &g.symbol == name
                                && g.definition_pointer.file_index == file_index
                                && g.definition_pointer.line_index == line_index
                        });

                    // Numeric labels and non-global internal names remain
                    // available to encoding without appearing in ELF metadata.
                    if name.chars().all(|c| c.is_ascii_digit())
                        || (!is_global && is_internal_symbol(name))
                    {
                        continue;
                    }

                    if !is_global {
                        let pointer = LinePointer { file_index, line_index };
                        let line_layout = self.layout.get(pointer);
                        let (addr, section_idx) = match line_layout.segment {
                            Segment::Text => (
                                text_start + line_layout.offset,
                                text_section_index,
                            ),
                            Segment::Data => {
                                let idx = data_section_index.ok_or_else(|| {
                                    RiscletError::internal(
                                        "Layout indicates data segment exists but no data section was created. \
                                         This indicates a bug in the assembler.".to_string()
                                    )
                                })?;
                                (data_start + line_layout.offset, idx)
                            }
                            Segment::Bss => {
                                let idx = bss_section_index.ok_or_else(|| {
                                    RiscletError::internal(
                                        "Layout indicates bss segment exists but no bss section was created. \
                                         This indicates a bug in the assembler.".to_string()
                                    )
                                })?;
                                (bss_start + line_layout.offset, idx)
                            }
                        };
                        let name_idx = self.symbol_names.add(name);

                        self.add_symbol(ElfSymbol {
                            st_name: name_idx,
                            st_info: make_st_info(STB_LOCAL, STT_NOTYPE),
                            st_other: 0,
                            st_shndx: section_idx,
                            st_value: addr,
                            st_size: 0,
                        });
                    }
                }
            }
        }

        // Add linker-provided symbols (all global)
        // These come before user-defined global symbols

        // __global_pointer$ = data_start + 0x800
        let gp_name = self.symbol_names.add("__global_pointer$");
        self.add_symbol(ElfSymbol {
            st_name: gp_name,
            st_info: make_st_info(STB_GLOBAL, STT_NOTYPE),
            st_other: 0,
            st_shndx: SHN_ABS,
            st_value: data_start + 0x800,
            st_size: 0,
        });

        // Add user-defined global symbols
        for global in &symbol_links.global_symbols {
            // Skip __global_pointer$ - it's already emitted above as a linker-provided symbol
            if global.symbol == "__global_pointer$" {
                continue;
            }

            let file_index = global.definition_pointer.file_index;
            let line_index = global.definition_pointer.line_index;

            let name_idx = self.symbol_names.add(&global.symbol);
            let def = SymbolDefinition {
                symbol: global.symbol.clone(),
                pointer: global.definition_pointer,
            };
            let value = symbol_values.get(&def).unwrap();
            let (st_value, st_shndx) = match value {
                EvaluatedValue::Integer(v) => (v as u32, SHN_ABS),
                EvaluatedValue::Address(a) => {
                    let pointer = LinePointer { file_index, line_index };
                    let line_layout = self.layout.get(pointer);
                    let section_idx = match line_layout.segment {
                        Segment::Text => text_section_index,
                        Segment::Data => data_section_index.ok_or_else(|| {
                            RiscletError::internal(
                                "Global symbol references data segment but no data section was created. \
                                 This indicates a bug in the assembler.".to_string()
                            )
                        })?,
                        Segment::Bss => bss_section_index.ok_or_else(|| {
                            RiscletError::internal(
                                "Global symbol references bss segment but no bss section was created. \
                                 This indicates a bug in the assembler.".to_string()
                            )
                        })?,
                    };
                    (a, section_idx)
                }
            };

            self.add_symbol(ElfSymbol {
                st_name: name_idx,
                st_info: make_st_info(STB_GLOBAL, STT_NOTYPE),
                st_other: 0,
                st_shndx,
                st_value,
                st_size: 0,
            });
        }

        Ok(())
    }
}

/// Create a new ELF header with standard RISC-V 32-bit values
fn executable_header() -> ElfHeader {
    let mut e_ident = [0u8; 16];
    e_ident[0] = EI_MAG0;
    e_ident[1] = EI_MAG1;
    e_ident[2] = EI_MAG2;
    e_ident[3] = EI_MAG3;
    e_ident[4] = EI_CLASS;
    e_ident[5] = EI_DATA;
    e_ident[6] = EI_VERSION;
    e_ident[7] = EI_OSABI;
    e_ident[8] = EI_ABIVERSION;

    ElfHeader {
        e_ident,
        e_type: ET_EXEC,
        e_machine: EM_RISCV,
        e_version: EV_CURRENT,
        e_entry: 0,
        e_phoff: ElfHeader::SIZE as u32,
        e_shoff: 0,
        e_flags: EF_RISCV_FLOAT_ABI_DOUBLE,
        e_ehsize: ElfHeader::SIZE as u16,
        e_phentsize: ElfProgramHeader::SIZE as u16,
        e_phnum: 0,
        e_shentsize: ElfSectionHeader::SIZE as u16,
        e_shnum: 0,
        e_shstrndx: 0,
    }
}

// ============================================================================
// String Table Builder
// ============================================================================

/// String table builder that deduplicates strings
struct StringTable {
    strings: Vec<u8>,
    offsets: HashMap<String, u32>,
}

impl StringTable {
    /// Create a new string table starting with a null byte
    fn new() -> Self {
        Self { strings: vec![0], offsets: HashMap::new() }
    }

    /// Add a string and return its offset
    fn add(&mut self, s: &str) -> u32 {
        if let Some(&offset) = self.offsets.get(s) {
            return offset;
        }

        let offset = self.strings.len() as u32;
        self.offsets.insert(s.to_string(), offset);
        self.strings.extend_from_slice(s.as_bytes());
        self.strings.push(0); // Null terminator
        offset
    }

    /// Get the raw bytes of the string table
    fn data(&self) -> &[u8] {
        &self.strings
    }

    /// Get the length of the string table
    fn len(&self) -> usize {
        self.strings.len()
    }
}

// ============================================================================
// RISC-V Attributes Section
// ============================================================================

/// Generate .riscv.attributes section content
///
/// This section describes the RISC-V ISA features used by the binary.
/// Format follows the ELF attributes specification with RISC-V extensions.
///
/// For RV32IMACZifencei (I, M, A, C extensions + Zifencei), we generate:
/// "rv32i2p1_m2p0_a2p1_c2p0_zifencei2p0"
fn generate_riscv_attributes() -> Vec<u8> {
    // Generate attributes for RV32IMAC with compressed instructions and Zifencei
    let arch_string = "rv32i2p1_m2p0_a2p1_c2p0_zifencei2p0";

    let mut attrs = Vec::new();

    // Format version (always 'A' = 0x41)
    attrs.push(b'A');

    // Total length of attribute section (will be patched)
    let length_pos = attrs.len();
    attrs.extend_from_slice(&[0u8; 4]);

    // Vendor name (always "riscv" for RISC-V)
    attrs.extend_from_slice(b"riscv\0");

    // File attributes tag (1)
    attrs.push(1);

    // Length of file attributes subsection (will be patched)
    let file_attrs_length_pos = attrs.len();
    attrs.extend_from_slice(&[0u8; 4]);

    // Tag_RISCV_arch (5): RISC-V architecture string
    attrs.push(5);
    attrs.extend_from_slice(arch_string.as_bytes());
    attrs.push(0); // Null terminator

    // Patch file attributes length
    let file_attrs_length = (attrs.len() - file_attrs_length_pos) as u32;
    attrs[file_attrs_length_pos..file_attrs_length_pos + 4]
        .copy_from_slice(&file_attrs_length.to_le_bytes());

    // Patch total length
    let total_length = (attrs.len() - length_pos) as u32;
    attrs[length_pos..length_pos + 4]
        .copy_from_slice(&total_length.to_le_bytes());

    attrs
}
