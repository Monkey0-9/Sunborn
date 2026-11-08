#!/usr/bin/env bash
# =================================================================
# Sunborn Enterprise Control Plane & Gateway Full-Stack Launcher
# =================================================================

set -e

echo -e "\033[1;36m=================================================================\033[0m"
echo -e "\033[1;37m   SUNBORN ENTERPRISE CONTROL PLANE & SEMANTIC GATEWAY LAUNCHER  \033[0m"
echo -e "\033[1;36m=================================================================\033[0m"

echo -e "\n\033[1;33m[1/3] Checking Sunborn Gateway (Port 8080)...\033[0m"
if curl -s http://127.0.0.1:8080/health | grep -q "healthy"; then
  echo -e "\033[1;32m      Sunborn Gateway is already running at http://127.0.0.1:8080\033[0m"
else
  echo -e "\033[1;36m      Starting Sunborn Gateway Axum Server...\033[0m"
  export SUNBORN_ENV="development"
  export SUNBORN_CACHE_BACKEND="memory"
  export SUNBORN_VECTOR_BACKEND="memory"
  export SUNBORN_DB_BACKEND="memory"
  cargo run --bin sunborn-gateway > /dev/null 2>&1 &
  GATEWAY_PID=$!
  echo "      Waiting for gateway to initialize (PID: $GATEWAY_PID)..."
  for i in {1..30}; do
    if curl -s http://127.0.0.1:8080/health | grep -q "healthy"; then
      echo -e "\033[1;32m      Gateway online!\033[0m"
      break
    fi
    sleep 0.5
  done
fi

echo -e "\n\033[1;33m[2/3] Checking Frontend Dependencies...\033[0m"
cd frontend
if [ ! -d "node_modules" ]; then
  echo -e "\033[1;36m      Installing node modules...\033[0m"
  npm install
fi

echo -e "\n\033[1;33m[3/3] Starting Next.js Enterprise Control Plane...\033[0m"
echo -e "\033[1;32m  -> UI:      http://localhost:3000\033[0m"
echo -e "\033[1;32m  -> Gateway: http://127.0.0.1:8080\033[0m"
npm run dev
