import httpx
import logging

async def fetch_user(user_id: str) -> Result[UserData, str]:
    try:
        resp = await httpx.AsyncClient().get(f"/users/{user_id}", timeout=5)
        resp.raise_for_status()
        return Ok(UserData(**resp.json()))
    except Exception as e:
        return Err(str(e))

async def send_email(to: str, subject: str, body: str) -> Result[None, str]:
    try:
        resp = await httpx.AsyncClient().post("/email/send", json={"to": to, "subject": subject, "body": body}, timeout=10)
        resp.raise_for_status()
        return Ok(None)
    except Exception as e:
        return Err(str(e))

async def notify_user(user_id: str, message: str) -> Result[None, str]:
    user_result = await fetch_user(user_id)
    if isinstance(user_result, Err):
        logging.error(f"Failed to fetch user: {user_result.error}")
        return Err(f"User lookup failed: {user_result.error}")
    data = user_result.value
    if not data.verified:
        return Err("User email not verified")
    sent = await send_email(to=data.email, subject="Notification", body=message)
    if isinstance(sent, Err):
        logging.error(f"Email failed: {sent.error}")
        return Err(f"Send failed: {sent.error}")
    logging.info(f"Notified user {user_id}")
    return Ok(None)
