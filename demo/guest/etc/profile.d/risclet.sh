HISTFILE=/tmp/risclet-history
MICRO_CONFIG_HOME=/etc/micro
export HISTFILE MICRO_CONFIG_HOME

# Login shells report cramped terminals without affecting nonterminal sessions.
if [ -t 0 ]; then
    risclet_size=$(stty size)
    risclet_rows=${risclet_size% *}
    risclet_cols=${risclet_size#* }
    if [ "$risclet_cols" -lt 80 ] || [ "$risclet_rows" -lt 24 ]; then
        printf 'warning: terminal is only %s×%s, risclet works best with 80×24 or larger\n' "$risclet_cols" "$risclet_rows"
    fi
    unset risclet_size risclet_rows risclet_cols
fi
