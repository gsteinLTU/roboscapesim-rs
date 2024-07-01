echo "Updating Caddyfile"

# Check if we are running in a container and source the environment variables
if [[ -f /proc/1/environ ]]; then
  echo "Sourcing environment variables"
  for I in `cat /proc/1/environ  | strings`; do echo "export $I"; done > /srv/.profile
  source /srv/.profile
  # print the environment variables
  cat /srv/.profile
  env
fi

# Get the current IP address in the DASH_IP format
DASH_IP="`curl -s http://checkip.amazonaws.com | tr . -`"
echo "DASH_IP: $DASH_IP"

# Copy the template to the actual Caddyfile
cp Caddyfile.template Caddyfile

# Replace the placeholder with the actual DASH_IP
sed -i -e "s/DASH_IP/$DASH_IP/" Caddyfile

# Check if CERT and KEY environment variables are set
if [[ -n "$CERT" && -n "$KEY" ]]; then
  echo "TLS certificate and key are provided"

  # Decode and write the CERT and KEY to files
  echo "$CERT" | base64 -d > cert.pem
  echo "$KEY" | base64 -d > key.pem

  # Verify the certificate is still valid
  EXPIRY_DATE=`openssl x509 -enddate -noout -in cert.pem | cut -d= -f2 | sed 's/ GMT//g'`
  if [[ `date -d "$EXPIRY_DATE" +%s` -lt `date +%s` ]]; then
    echo "Certificate has expired"
    sed -i -e "s/EXTRA//" Caddyfile
  else
    echo "Certificate is still valid"
    # Replace the placeholder with TLS configuration
    sed -i -e "s/EXTRA/tls cert.pem key.pem/" Caddyfile
  fi
else
  echo "TLS certificate and key are not provided"
  # Replace the placeholder with no TLS configuration
  sed -i -e "s/EXTRA//" Caddyfile
fi