# Multi-stage build for Astro static site
FROM node:22-alpine AS builder

WORKDIR /app

# Copy package files
COPY package*.json ./

RUN npm ci

# Copy source files
COPY . .

# Build static site with translations
RUN npm run build

# Production stage - serve with Nginx
FROM nginx:alpine

# Copy custom nginx configuration
COPY <<'EOF' /etc/nginx/conf.d/default.conf
server {
    listen 80;
    server_name _;

    root /usr/share/nginx/html;
    index index.html;

    # Coolify/aaPanel terminates TLS in front.
    absolute_redirect off;
    port_in_redirect off;

    # Gzip Compression
    gzip on;
    gzip_vary on;
    gzip_min_length 1024;
    gzip_proxied expired no-cache no-store private auth;
    gzip_types text/plain text/css text/xml text/javascript application/x-javascript application/xml application/javascript application/json image/svg+xml;

    # Canonical paths have no trailing slash (matches sitemap/hreflang).
    location ~ ^(.+)/$ {
        return 301 $1;
    }

    location / {
        # Do NOT use $uri/ — forces http://host/path/ on :80.
        # No /index.html catch-all (soft-404).
        try_files $uri $uri.html $uri/index.html =404;
    }

    # Cache static assets
    location ~* \.(?:css|js|jpg|jpeg|gif|png|ico|cur|gz|svg|svgz|mp4|ogg|ogv|webm|htc|woff|woff2|ttf)\$ {
        expires 1y;
        add_header Cache-Control "public, must-revalidate, proxy-revalidate";
        try_files $uri =404;
    }

    error_page 404 /404.html;
}
EOF

# Copy dist files from builder
COPY --from=builder /app/dist/client /usr/share/nginx/html
COPY --from=builder /app/dist/ru /usr/share/nginx/html/ru

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
