from fastapi import Request, HTTPException
from sqlmodel import Session, select

async def req_to_data(req: Request):
    if req.headers.get("Content-Type") == "application/json":
        data = await req.json()
    elif req.headers.get("Content-Type") in [
        "multipart/form-data",
        "application/x-www-form-urlencoded",
    ]:
        data = dict(await req.form())
    else:
        raise HTTPException(status_code=422, detail="Invalid Content-Type")
    return data
