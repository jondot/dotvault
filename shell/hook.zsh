_dotvault_hook() {
  if [[ -f .dotvault.toml ]] || [[ -f .dotvault.local.toml ]]; then
    eval "$(dv export)"
  fi
}
autoload -U add-zsh-hook
add-zsh-hook chpwd _dotvault_hook
_dotvault_hook
