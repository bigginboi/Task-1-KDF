FROM gcc:latest

RUN apt-get update && apt-get install -y unzip zip curl && rm -rf /var/lib/apt/lists/*
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

RUN mkdir -p /var/code

CMD ["tail", "-f", "/dev/null"]
