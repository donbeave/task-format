#!/usr/bin/env bash
# init-firewall.sh — default-DROP egress firewall with an ipset domain allowlist.
#
#   ALLOWED_DOMAINS="api.anthropic.com api.openai.com" /usr/local/bin/init-firewall.sh
#
# Concept from anthropics/claude-code .devcontainer/init-firewall.sh (domains parameterised).
# UNUSED under DinD v1 (D18): the inner dockerd manages iptables itself and the prereq layer
# pulls nothing at runtime, so v1 runs with unrestricted egress (NET_MODE=all). Kept for a
# future NET_MODE=api variant. Requires the container to run --cap-add NET_ADMIN --cap-add
# NET_RAW. NOTE: traffic from inner-docker containers traverses FORWARD, not OUTPUT — a
# DinD-safe version must also allow ESTABLISHED + the ipset on the FORWARD chain.
set -Eeuo pipefail

[ "$(id -u)" -eq 0 ] || { echo "init-firewall.sh: must run as root" >&2; exit 1; }
ALLOWED_DOMAINS="${ALLOWED_DOMAINS:-api.anthropic.com}"

# resolve each allowlisted domain into the `allowed-domains` ipset
ipset create allowed-domains hash:ip -exist
for domain in $ALLOWED_DOMAINS; do
  echo "allow domain: $domain"
  getent ahostsv4 "$domain" | awk '{print $1}' | sort -u | while IFS= read -r ip; do
    ipset add allowed-domains "$ip" -exist
  done
done

# allow loopback, established, DNS, and the ipset; drop every other outbound packet
iptables -A OUTPUT -o lo -j ACCEPT
iptables -A OUTPUT -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT
iptables -A OUTPUT -p udp --dport 53 -j ACCEPT
iptables -A OUTPUT -p tcp --dport 53 -j ACCEPT
iptables -A OUTPUT -m set --match-set allowed-domains dst -j ACCEPT
iptables -P OUTPUT DROP

# positive + negative checks — fail loudly if the allowlist is not in effect
ok=0
curl -fsSI --max-time 10 "https://${ALLOWED_DOMAINS%% *}" >/dev/null && ok=1 \
  || echo "WARN: allowlisted domain unreachable" >&2
if curl -fsSI --max-time 10 https://example.com >/dev/null 2>&1; then
  echo "WARN: non-allowlisted domain still reachable — firewall not effective" >&2
fi
[ "$ok" -eq 1 ]
