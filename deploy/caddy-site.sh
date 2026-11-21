# shellcheck shell=bash
# Caddy site configuration used by deploy/install.sh (sourced, not run directly).
# It never overwrites a Caddyfile that serves other sites.

# True when the file holds only the distribution's sample site (":80 { root ... file_server }").
caddy_is_stock() {
  local content expected
  content="$(sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' -e '/^#/d' -e '/^$/d' "$1")"
  expected="$(printf '%s\n' ':80 {' 'root * /usr/share/caddy' 'file_server' '}')"
  [ "$content" = "$expected" ]
}

# True when a non-comment line of the file mentions the domain as a whole word.
caddy_has_site() {
  local re="${2//./\\.}" body
  body="$(grep -vE '^[[:space:]]*#' "$1" || true)"   # captured first: grep -q exiting early would make the writer fail
  grep -qE "(^|[[:space:],])${re}([[:space:],{]|\$)" <<<"$body"
}

# caddy_apply_site CADDYFILE DOMAIN PORT TEMPLATE
#   Writes the template for DOMAIN (proxying to 127.0.0.1:PORT) when the file is absent or stock,
#   appends it when the file has other sites and no block for DOMAIN, and leaves the file alone
#   (with a warning) when DOMAIN is already configured. The new file is assembled beside the old one
#   (CADDYFILE.nf-new), checked with 'caddy validate', and only then renamed into place, so an interruption or an invalid
#   result leaves the old file as it was. Every change is backed up to CADDYFILE.bak-<timestamp> first.
#   Sets CADDY_CHANGED=1 when the file was written. Returns 1 when the result was refused.
#   Does not reload Caddy; the caller does that.
caddy_apply_site() {
  local cf="$1" dom="$2" port="$3" tpl="$4" block mode bak="" out new
  CADDY_CHANGED=0
  block="$(sed -e "s|app\.example\.com|${dom}|g" -e "s|127\.0\.0\.1:8787|127.0.0.1:${port}|g" -e '/^#/d' "$tpl")"

  if [ ! -f "$cf" ] || caddy_is_stock "$cf"; then
    mode=replace
  elif caddy_has_site "$cf" "$dom"; then
    echo "WARNING: $cf already has a block for $dom; left unchanged. Check that it proxies to 127.0.0.1:${port}." >&2
    return 0
  else
    mode=append
  fi

  new="$cf.nf-new"
  rm -f "$new"
  if [ -f "$cf" ]; then cp -p "$cf" "$new"; fi  # keeps the owner and mode of the file it replaces
  if [ "$mode" = replace ]; then
    printf '%s\n' "$block" > "$new"
  else
    printf '\n%s\n' "$block" >> "$new"
  fi
  if ! out="$(caddy validate --config "$new" --adapter caddyfile 2>&1)"; then
    echo "new Caddyfile does not validate:" >&2
    echo "$out" >&2
    rm -f "$new"
    echo "$cf was left unchanged" >&2
    return 1
  fi

  if [ -f "$cf" ]; then
    bak="$cf.bak-$(date +%Y%m%d%H%M%S)"
    cp -p "$cf" "$bak"
    echo "backed up $cf to $bak"
  fi
  mv -f "$new" "$cf"
  if [ "$mode" = replace ]; then
    echo "wrote the site block for $dom to $cf"
  else
    echo "appended the site block for $dom to $cf (other sites kept)"
  fi
  # shellcheck disable=SC2034  # read by deploy/install.sh
  CADDY_CHANGED=1
}
