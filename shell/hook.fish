function _dotvault_hook --on-variable PWD
  if test -f .dotvault.toml; or test -f .dotvault.local.toml
    eval (dv export)
  end
end
_dotvault_hook
