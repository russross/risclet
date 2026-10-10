" Run from the repository root: vim -Nu NONE -n -es -S syntaxhighlighting/vim/test.vim
" Exercise filename detection independently of Vim's bundled assembly detectors.
source syntaxhighlighting/vim/ftdetect/riscv.vim
doautocmd BufNewFile example.s
call assert_equal('riscv', &filetype)
setlocal filetype=
doautocmd BufRead example.s
call assert_equal('riscv', &filetype)

" Shared fixtures use byte columns and editor-specific expected token names.
source syntaxhighlighting/vim/syntax/riscv.vim
let s:cases = json_decode(join(readfile('syntaxhighlighting/tests/highlighting.json'), "\n"))
call setline(1, map(copy(s:cases), 'v:val.line'))

" Check actual syntax groups, including line recovery after malformed literals.
for s:index in range(len(s:cases))
  for s:check in s:cases[s:index].checks
    call assert_equal('riscv' . s:check[1],
          \ synIDattr(synID(s:index + 1, s:check[0] + 1, 1), 'name'),
          \ s:cases[s:index].line)
  endfor
endfor
if !empty(v:errors)
  call writefile(v:errors, '/dev/stderr')
  cquit
endif
qa!
