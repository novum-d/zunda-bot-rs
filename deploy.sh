#!/usr/bin/env bash
set -eu

if [ ! -f .env ]; then
  echo ".env が見つからないためデプロイできません" >&2
  exit 1
fi

set -a
. ./.env
set +a

: "${GCP_PROJECT_ID:?GCP_PROJECT_ID を .env に設定してください}"
: "${GCP_REGION:?GCP_REGION を .env に設定してください}"
: "${ARTIFACT_REGISTRY_REPOSITORY:?ARTIFACT_REGISTRY_REPOSITORY を .env に設定してください}"
: "${CLOUD_RUN_SERVICE:?CLOUD_RUN_SERVICE を .env に設定してください}"
: "${CONTAINER_IMAGE_NAME:?CONTAINER_IMAGE_NAME を .env に設定してください}"

CONTAINER_IMAGE_TAG="${CONTAINER_IMAGE_TAG:-latest}"
ENABLE_DISCORD_BOT="${ENABLE_DISCORD_BOT:-true}"
IMAGE_URI="${GCP_REGION}-docker.pkg.dev/${GCP_PROJECT_ID}/${ARTIFACT_REGISTRY_REPOSITORY}/${CONTAINER_IMAGE_NAME}:${CONTAINER_IMAGE_TAG}"

gcloud builds submit \
  --tag "${IMAGE_URI}" \
  --project "${GCP_PROJECT_ID}"

gcloud run deploy "${CLOUD_RUN_SERVICE}" \
  --image "${IMAGE_URI}" \
  --region "${GCP_REGION}" \
  --platform managed \
  --allow-unauthenticated \
  --set-secrets DISCORD_TOKEN=DISCORD_TOKEN:latest,DATABASE_URL=DATABASE_URL:latest \
  --set-env-vars "ENABLE_DISCORD_BOT=${ENABLE_DISCORD_BOT}" \
  --project "${GCP_PROJECT_ID}"
