_dotvault_hook() {
  if [[ -f .dotvault.toml ]] || [[ -f .dotvault.local.toml ]]; then
    eval "$(dv export)"
  fi
}
PROMPT_COMMAND="_dotvault_hook;$PROMPT_COMMAND"
