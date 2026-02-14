# Install dependencies
FROM node:20-slim AS build

WORKDIR /usr/src/app

COPY package.json package-lock.json ./
RUN npm ci --production && npm cache clean --force

# Production image
FROM node:20-slim

ARG VERSION=dev
ENV APP_VERSION=$VERSION
ENV NODE_ENV=production

# Runtime secrets — injected via docker run -e or orchestrator
# These MUST be provided at runtime, never baked into the image
ENV APP_ID=
ENV PRIVATE_KEY=
ENV WEBHOOK_SECRET=
ENV JULES_API_KEY=
ENV WEBHOOK_PROXY_URL=

WORKDIR /usr/src/app

# Copy production node_modules from build stage
COPY --from=build /usr/src/app/node_modules ./node_modules

# Copy application source
COPY package.json package-lock.json ./
COPY index.js app.js ./
COPY lib ./lib

# Run as non-root user for security
USER node

EXPOSE 3000

LABEL org.opencontainers.image.source="https://github.com/Pawgloo/bot"
LABEL org.opencontainers.image.description="Pawgloo GitHub App"

CMD ["npm", "start"]
