" Select the risclet dialect for assembly source files.
augroup risclet_filetype
  autocmd!
  autocmd BufRead,BufNewFile *.s,*.asm setfiletype risclet
augroup END
