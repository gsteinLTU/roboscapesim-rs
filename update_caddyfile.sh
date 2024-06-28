#!/bin/bash

# Get the current IP address in the DASH_IP format
DASH_IP="`curl -s http://checkip.amazonaws.com | tr . -`"

# Copy the template to the actual Caddyfile
cp Caddyfile.template Caddyfile

# Replace the placeholder with the actual DASH_IP
sed -i -e "s/DASH_IP/$DASH_IP/" Caddyfile

# Check if CERT and KEY environment variables are set
if [[ -n "$CERT" && -n "$KEY" ]]; then
  # Append the TLS configuration to the Caddyfile
  echo "
tls {
    certificate \"$CERT\"
    key \"$KEY\"
}
" >> Caddyfile
fi