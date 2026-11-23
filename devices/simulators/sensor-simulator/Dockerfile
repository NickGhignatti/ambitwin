FROM node:24.21.0-alpine AS builder
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM node:24.21.0-alpine AS production
WORKDIR /app
# Patch base-image OS CVEs so the cd-registry Trivy scan (OS-scoped) stays green.
RUN apk upgrade --no-cache
COPY package*.json ./
RUN npm ci --omit=dev \
    && npm cache clean --force
COPY --from=builder /app/dist ./dist
EXPOSE 3000
CMD ["npm", "start"]
