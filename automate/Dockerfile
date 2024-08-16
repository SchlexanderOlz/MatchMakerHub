FROM python:3.11-slim
WORKDIR /usr/src/automate

COPY ./auto.py .
COPY ./requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt