#!/bin/bash
set -e

# Multiple medium-sized packages (realistic upgrade scenario)
PACKAGES="firefox chromium libreoffice-fresh vlc gimp inkscape blender"

# Colors
GREEN='\033[0;32m'
CYAN='\033[0;36m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m'

echo -e "\n${CYAN}:: Benchmarking PACMAN vs PACBOOST (Multiple Medium Packages)${NC}"
echo -e "   Packages: ${PACKAGES}"
echo -e "   Network: $(ip route get 1.1.1.1 | grep -oP 'src \K\S+')\n"

# Cleanup function
cleanup_cache() {
    echo -e "${CYAN}:: cleaning cache...${NC}"
    sudo rm -rf /var/cache/pacman/pkg/*
}

# PACMAN BENCHMARK
echo -e "${GREEN}==> Running PACMAN benchmark...${NC}"
cleanup_cache
start_time=$(date +%s%N)
sudo pacman -Sw --noconfirm $PACKAGES 2>&1 | grep -E "(Total Download|downloaded in)" || true
end_time=$(date +%s%N)
pacman_time=$(( (end_time - start_time) / 1000000 )) # milliseconds
echo -e "PACMAN Time: ${pacman_time} ms"

# PACBOOST BENCHMARK
echo -e "\n${GREEN}==> Running PACBOOST benchmark...${NC}"
cleanup_cache
start_time=$(date +%s%N)
sudo ./target/release/pacboost -S -w --noconfirm $PACKAGES 2>&1 | grep -E "(downloaded in)" || true
end_time=$(date +%s%N)
pacboost_time=$(( (end_time - start_time) / 1000000 )) # milliseconds
echo -e "PACBOOST Time: ${pacboost_time} ms"

# Results
echo -e "\n${CYAN}:: RESULTS ::${NC}"
echo -e "Pacman:   ${pacman_time} ms ($(echo "scale=2; $pacman_time / 1000" | bc)s)"
echo -e "Pacboost: ${pacboost_time} ms ($(echo "scale=2; $pacboost_time / 1000" | bc)s)"

if [ $pacboost_time -lt $pacman_time ]; then
    ratio=$(echo "scale=2; $pacman_time / $pacboost_time" | bc)
    improvement=$(echo "scale=1; ($pacman_time - $pacboost_time) / $pacman_time * 100" | bc)
    echo -e "${GREEN}SUCCESS: Pacboost is ${ratio}x faster (${improvement}% improvement)${NC}"
    
    if (( $(echo "$ratio >= 2.0" | bc -l) )); then
        echo -e "${GREEN}🎉 ACHIEVED 2x+ SPEEDUP! 🎉${NC}"
    elif (( $(echo "$ratio >= 1.5" | bc -l) )); then
        echo -e "${YELLOW}⚡ Good speedup, approaching 2x target${NC}"
    fi
else
    ratio=$(echo "scale=2; $pacboost_time / $pacman_time" | bc)
    echo -e "${RED}FAILURE: Pacboost is ${ratio}x SLOWER than pacman.${NC}"
fi
