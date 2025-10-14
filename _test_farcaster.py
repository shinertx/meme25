import os
import requests
import json

api_key = os.getenv("FARCASTER_API_KEY")
if not api_key:
    print("--- ERROR ---")
    print("FARCASTER_API_KEY not found in environment.")
    exit(1)

url = "https://api.neynar.com/v2/farcaster/user/bulk"
params = { "fids": "1" }
headers = {
    "Content-Type": "application/json",
    "api_key": api_key
}

print(f"--- Testing Farcaster (Neynar) API Key ---")

try:
    resp = requests.get(url, headers=headers, params=params, timeout=10)
    print(f"Status Code: {resp.status_code}\n")
    print("--- Response Body ---\n")
    print(resp.text)
    
    if resp.status_code == 200 and "dwr.eth" in resp.text:
        print("\n--- VERDICT: ✅ VALID ---")
    else:
        print(f"\n--- VERDICT: ❌ INVALID ---")
        print(f"Neynar responded with status {resp.status_code}. The key may be incorrect or lack permissions.")

except requests.exceptions.RequestException as e:
    print(f"--- REQUEST FAILED ---")
    print(f"An error occurred: {e}")
    print("\n--- VERDICT: ❌ UNKNOWN (Network Error) ---")
