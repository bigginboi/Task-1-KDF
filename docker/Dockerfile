FROM gcc:latest

WORKDIR /src

# Default build script: looks for Makefile, else compiles all .cpp files
CMD if [ -f Makefile ]; then \
      mkdir -p build && make -j$(nproc) && echo "Build completed via Makefile"; \
    elif ls *.cpp 1>/dev/null 2>&1 || find . -name '*.cpp' | grep -q .; then \
      mkdir -p build && \
      find . -name '*.cpp' -not -path './build/*' | head -20 | xargs g++ -o build/output && \
      echo "Build completed: build/output"; \
    elif ls src/*.cpp 1>/dev/null 2>&1; then \
      mkdir -p build && \
      g++ -o build/output src/*.cpp && \
      echo "Build completed: build/output"; \
    else \
      echo "No compilable source files found" && exit 1; \
    fi
