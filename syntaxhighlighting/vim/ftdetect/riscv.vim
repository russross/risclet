" Select the riscv dialect for assembly source files.
augroup riscv_filetype
  autocmd!
  autocmd BufRead,BufNewFile *.s setfiletype riscv
augroup END
