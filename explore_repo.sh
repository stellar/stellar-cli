#!/bin/bash
# Explore the stellar-cli repository structure for simulation-related code
find . -type f -name "*.rs" | xargs grep -l "simulation\|simulate\|SimulationResult\|Budget\|cpu_instructions\|footprint" 2>/dev/null | head -30
