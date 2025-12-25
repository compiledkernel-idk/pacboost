#!/bin/bash
set -e

# Target package to benchmark
PKG="cuda"

# Colors
GREEN='\033[0;32m'
CYAN='\033[0;36m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "\n${CYAN}:: Benchmarking PACMAN vs PACBOOST${NC}"
echo -e "   Target: ${PKG}"
echo -e "   Network: $(ip route get 1.1.1.1 | grep -oP 'src \K\S+')\n"

# Cleanup function
cleanup_cache() {
    echo -e "${CYAN}:: cleaning cache for $PKG...${NC}"
    # Remove from pacman cache forcibly
    sudo rm -f /var/cache/pacman/pkg/${PKG}-*
    sudo rm -f /var/cache/pacman/pkg/${PKG}.*
}

# PACMAN BENCHMARK
echo -e "${GREEN}==> Running PACMAN benchmark...${NC}"
cleanup_cache
start_time=$(date +%s%N)
sudo pacman -Sw --noconfirm $PKG
end_time=$(date +%s%N)
pacman_time=$(( (end_time - start_time) / 1000000 )) # milliseconds
echo -e "PACMAN Time: ${pacman_time} ms"

# PACBOOST BENCHMARK
echo -e "\n${GREEN}==> Running PACBOOST benchmark...${NC}"
cleanup_cache
start_time=$(date +%s%N)
# Clean benchmark (download only)
sudo ./target/release/pacboost -S -w --noconfirm $PKG
end_time=$(date +%s%N)
pacboost_time=$(( (end_time - start_time) / 1000000 )) # milliseconds
echo -e "PACBOOST Time: ${pacboost_time} ms"

# Results
echo -e "\n${CYAN}:: RESULTS ::${NC}"
echo -e "Pacman:   ${pacman_time} ms"
echo -e "Pacboost: ${pacboost_time} ms"

if [ $pacboost_time -lt $pacman_time ]; then
    ratio=$(echo "scale=2; $pacman_time / $pacboost_time" | bc)
    echo -e "${GREEN}SUCCESS: Pacboost is ${ratio}x faster than pacman!${NC}"
else
    ratio=$(echo "scale=2; $pacboost_time / $pacman_time" | bc)
    echo -e "${RED}FAILURE: Pacboost is ${ratio}x SLOWER than pacman.${NC}"
fi
