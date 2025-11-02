#[cfg(test)]
mod riscv_isa_tests {
    use crate::io_abstraction::TestIo;
    use crate::riscv::Op;

    const MAX_STEPS: usize = 100000;

    fn run_test_binary(binary: &[u8]) -> Result<i32, String> {
        let io = TestIo::new();
        let mut machine = crate::elf::load_elf_from_bytes(binary, Box::new(io))?;

        for _step in 0..MAX_STEPS {
            let pc = machine.pc();

            let (raw_instruction, length) =
                machine.load_instruction(pc).map_err(|e| format!("Failed to load instruction at {:x}: {}", pc, e))?;

            let op = Op::new(raw_instruction);
            let is_ecall = matches!(op, Op::Ecall);

            machine.execute_and_collect_effects(&std::rc::Rc::new(crate::execution::Instruction {
                address: pc,
                op,
                length,
                pseudo_index: 0,
                verbose_fields: Vec::new(),
                pseudo_fields: Vec::new(),
            }));

            if is_ecall {
                let exit_code = machine.get_reg(10);
                return Ok(exit_code);
            }
        }

        Err(format!("Test did not complete within {} steps", MAX_STEPS))
    }

    macro_rules! riscv_test {
        ($test_name:ident, $binary:expr) => {
            #[test]
            fn $test_name() {
                let exit_code = run_test_binary($binary).expect("Test execution failed");

                assert_eq!(exit_code, 0, "Test failed with exit code {}", exit_code);
            }
        };
    }

    riscv_test!(test_add, include_bytes!("test_binaries/add"));
    riscv_test!(test_addi, include_bytes!("test_binaries/addi"));
    riscv_test!(test_and, include_bytes!("test_binaries/and"));
    riscv_test!(test_andi, include_bytes!("test_binaries/andi"));
    riscv_test!(test_auipc, include_bytes!("test_binaries/auipc"));
    riscv_test!(test_beq, include_bytes!("test_binaries/beq"));
    riscv_test!(test_bge, include_bytes!("test_binaries/bge"));
    riscv_test!(test_bgeu, include_bytes!("test_binaries/bgeu"));
    riscv_test!(test_blt, include_bytes!("test_binaries/blt"));
    riscv_test!(test_bltu, include_bytes!("test_binaries/bltu"));
    riscv_test!(test_bne, include_bytes!("test_binaries/bne"));
    riscv_test!(test_div, include_bytes!("test_binaries/div"));
    riscv_test!(test_divu, include_bytes!("test_binaries/divu"));
    riscv_test!(test_jal, include_bytes!("test_binaries/jal"));
    riscv_test!(test_jalr, include_bytes!("test_binaries/jalr"));
    riscv_test!(test_lb, include_bytes!("test_binaries/lb"));
    riscv_test!(test_lbu, include_bytes!("test_binaries/lbu"));
    riscv_test!(test_ld_st, include_bytes!("test_binaries/ld_st"));
    riscv_test!(test_lh, include_bytes!("test_binaries/lh"));
    riscv_test!(test_lhu, include_bytes!("test_binaries/lhu"));
    riscv_test!(test_lui, include_bytes!("test_binaries/lui"));
    riscv_test!(test_lw, include_bytes!("test_binaries/lw"));
    riscv_test!(test_ma_data, include_bytes!("test_binaries/ma_data"));
    riscv_test!(test_mul, include_bytes!("test_binaries/mul"));
    riscv_test!(test_mulh, include_bytes!("test_binaries/mulh"));
    riscv_test!(test_mulhsu, include_bytes!("test_binaries/mulhsu"));
    riscv_test!(test_mulhu, include_bytes!("test_binaries/mulhu"));
    riscv_test!(test_or, include_bytes!("test_binaries/or"));
    riscv_test!(test_ori, include_bytes!("test_binaries/ori"));
    riscv_test!(test_rem, include_bytes!("test_binaries/rem"));
    riscv_test!(test_remu, include_bytes!("test_binaries/remu"));
    riscv_test!(test_rvc, include_bytes!("test_binaries/rvc"));
    riscv_test!(test_sb, include_bytes!("test_binaries/sb"));
    riscv_test!(test_sh, include_bytes!("test_binaries/sh"));
    riscv_test!(test_simple, include_bytes!("test_binaries/simple"));
    riscv_test!(test_sll, include_bytes!("test_binaries/sll"));
    riscv_test!(test_slli, include_bytes!("test_binaries/slli"));
    riscv_test!(test_slt, include_bytes!("test_binaries/slt"));
    riscv_test!(test_slti, include_bytes!("test_binaries/slti"));
    riscv_test!(test_sltiu, include_bytes!("test_binaries/sltiu"));
    riscv_test!(test_sltu, include_bytes!("test_binaries/sltu"));
    riscv_test!(test_sra, include_bytes!("test_binaries/sra"));
    riscv_test!(test_srai, include_bytes!("test_binaries/srai"));
    riscv_test!(test_srl, include_bytes!("test_binaries/srl"));
    riscv_test!(test_srli, include_bytes!("test_binaries/srli"));
    riscv_test!(test_st_ld, include_bytes!("test_binaries/st_ld"));
    riscv_test!(test_sub, include_bytes!("test_binaries/sub"));
    riscv_test!(test_sw, include_bytes!("test_binaries/sw"));
    riscv_test!(test_xor, include_bytes!("test_binaries/xor"));
    riscv_test!(test_xori, include_bytes!("test_binaries/xori"));

    // A extension - atomic instructions
    riscv_test!(test_amoadd_w, include_bytes!("test_binaries/amoadd_w"));
    riscv_test!(test_amoand_w, include_bytes!("test_binaries/amoand_w"));
    riscv_test!(test_amoor_w, include_bytes!("test_binaries/amoor_w"));
    riscv_test!(test_amoxor_w, include_bytes!("test_binaries/amoxor_w"));
    riscv_test!(test_amoswap_w, include_bytes!("test_binaries/amoswap_w"));
    riscv_test!(test_amomin_w, include_bytes!("test_binaries/amomin_w"));
    riscv_test!(test_amomax_w, include_bytes!("test_binaries/amomax_w"));
    riscv_test!(test_amominu_w, include_bytes!("test_binaries/amominu_w"));
    riscv_test!(test_amomaxu_w, include_bytes!("test_binaries/amomaxu_w"));
    riscv_test!(test_lrsc, include_bytes!("test_binaries/lrsc"));
}
