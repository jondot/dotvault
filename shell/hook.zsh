_dotvault_hook() {
  if [[ -f .dotvault.toml ]] || [[ -f .dotvault.local.toml ]]; then
    eval "$(dotvault export)"
  fi
}
autoload -U add-zsh-hook
add-zsh-hook chpwd _dotvault_hook
_dotvault_hook
