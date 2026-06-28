FROM python:3.14-slim

WORKDIR /app
COPY --from=ghcr.io/astral-sh/uv:latest /uv /uvx /bin/
COPY pyproject.toml uv.lock README.md /app/

# First copy the lock, install without source code
RUN uv sync --frozen --no-install-project

# Copy the app (if code changes we avoid installing heavy dependencies
COPY src /app/src

# Now sync again (necessary? idk)
RUN uv sync --frozen

CMD ["uv", "run", "fastapi", "run", "src/intime_embed/main.py", "--port", "80"]
