_dotvault_hook() {
  if [[ -f .dotvault.toml ]] || [[ -f .dotvault.local.toml ]]; then
    eval "$(dotvault export)"
  fi
}
PROMPT_COMMAND="_dotvault_hook;$PROMPT_COMMAND"
