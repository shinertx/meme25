from fastapi import FastAPI, HTTPException, BackgroundTasks
from pydantic import BaseModel
from typing import Dict, List, Optional, Any
import pandas as pd
import numpy as np
from datetime import datetime, timedelta
import asyncio
import redis.asyncio as redis
import httpx
import json
import uuid
import os
from scipy import stats
import ta

app = FastAPI(title="MemeSnipe Backtest Engine", version="1.0.0")

# Redis connection
redis_client = redis.from_url("redis://redis:6379", decode_responses=True)

# Birdeye API client
BIRDEYE_API_KEY = os.getenv("BIRDEYE_API_KEY", "")
BIRDEYE_API_URL = "https://public-api.birdeye.so"