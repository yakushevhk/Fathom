FROM mcr.microsoft.com/playwright:v1.52.0-jammy

WORKDIR /app
COPY package.json package-lock.json tsconfig.json ./
RUN npm ci --ignore-scripts
COPY src ./src

ENV COMPUTER_HOST=0.0.0.0 \
    COMPUTER_PORT=8765 \
    COMPUTER_WORKSPACE=/data/browser \
    COMPUTER_HEADLESS=true
VOLUME ["/data/browser"]
EXPOSE 8765

CMD ["npx", "tsx", "src/main.ts"]
