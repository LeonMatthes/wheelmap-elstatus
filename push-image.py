#!/usr/bin/env python3
import requests

mac = "0000021F19003B1D"   # destination mac address
dither = 0   # set dither to 1 is you're sending photos etc
apip = "192.168.100.191"   # ip address of your access point

# Prepare the HTTP POST request
url = "http://" + apip + "/imgupload"
payload = {"dither": dither, "mac": mac}  # Additional POST parameter
files = {"file": open("./elstatus.jpg", "rb")}  # File to be uploaded

# Send the HTTP POST request
response = requests.post(url, data=payload, files=files)

# Check the response status
if response.status_code == 200:
    print("Image uploaded successfully!")
else:
    print("Failed to upload the image: ." + response.status_code)

