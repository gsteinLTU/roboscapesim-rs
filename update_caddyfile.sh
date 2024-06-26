DASH_IP="`curl -s http://checkip.amazonaws.com | cut -d " " -f 2 | tr . -`"

cp Caddyfile.template Caddyfile
sed -i -e  "s/DASH_IP/$DASH_IP/g" Caddyfile
sed -i -e  "s/ZEROSSL_API_KEY/$ZEROSSL_API_KEY/g" Caddyfile
