# Repo2RLEnv bootstrap recipe for delightful-ai/leaven.
#
# Build from a leaven checkout:
#   docker build -f envs/delightful-ai__leaven/7061a10e55a1__63203e22/Dockerfile \
#     -t local/r2e-bootstrap/delightful-ai__leaven:7061a10e55a1-nonroot .
#
# Leaven has optional path dependencies that Cargo still parses, so the
# absolute /vendor tree must exist even when those features are disabled.
FROM rust:1-slim

ENV DEBIAN_FRONTEND=noninteractive
ENV PATH=/usr/local/cargo/bin:$PATH

RUN apt-get update -qq \
    && apt-get install -y -qq --no-install-recommends \
        ca-certificates \
        git \
        build-essential \
        pkg-config \
        libssl-dev \
        python3 \
        python3-pip \
        python3-venv \
    && rm -rf /var/lib/apt/lists/*

RUN python3 -m pip install --break-system-packages --no-cache-dir uv

RUN mkdir -p /vendor/github.com/openai /vendor/github.com/apple \
    && git clone --depth 1 https://github.com/openai/codex.git /vendor/github.com/openai/codex \
    && git clone --depth 1 https://github.com/delightful-ai/firkin.git /vendor/github.com/apple/containerization

WORKDIR /workspace
COPY . /workspace

RUN git rev-parse --is-inside-work-tree \
    && git remote set-url origin https://github.com/delightful-ai/leaven.git \
    && (git config --local --unset-all credential.helper || true) \
    && (git config --local --unset-all credential.https://github.com.helper || true) \
    && (git config --local --unset-all credential.https://gist.github.com.helper || true)

RUN useradd --create-home --shell /bin/bash leaven \
    && echo 'export PATH=/usr/local/cargo/bin:$PATH' > /etc/profile.d/cargo-path.sh \
    && chown -R leaven:leaven /workspace /vendor /usr/local/cargo

USER leaven

RUN cargo fetch
RUN uv run --with openpyxl python -c 'import openpyxl'
RUN timeout 3000 cargo test --workspace --no-run

CMD ["bash", "-lc", "export PATH=/usr/local/cargo/bin:$PATH; cargo test --workspace"]
