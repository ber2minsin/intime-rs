FROM python:3.12-slim

WORKDIR /app
RUN pip install --no-cache-dir fastapi==0.115.12 uvicorn[standard]==0.34.2 python-multipart==0.0.20
COPY main.py .

EXPOSE 80
CMD ["uvicorn", "main:app", "--host", "0.0.0.0", "--port", "80"]
