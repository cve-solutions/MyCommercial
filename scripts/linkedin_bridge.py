#!/usr/bin/env python3
"""
LinkedIn bridge for MyCommercial.
Called from Rust via subprocess with JSON stdin/stdout.

Usage: echo '{"action":"search","email":"...","password":"...","params":{...}}' | python3 linkedin_bridge.py
"""

import json
import sys
import os

def main():
    try:
        input_data = json.loads(sys.stdin.read())
    except json.JSONDecodeError as e:
        print(json.dumps({"error": f"Invalid JSON input: {e}"}))
        sys.exit(1)

    action = input_data.get("action", "")
    email = input_data.get("email", "")
    password = input_data.get("password", "")
    params = input_data.get("params", {})

    if not email or not password:
        print(json.dumps({"error": "email and password required"}))
        sys.exit(1)

    try:
        from linkedin_api import Linkedin
    except ImportError:
        print(json.dumps({"error": "linkedin-api not installed. Run: pip3 install linkedin-api"}))
        sys.exit(1)

    # Use cookies cache to avoid re-login each time
    cookies_dir = os.path.expanduser("~/.local/share/mycommercial")
    os.makedirs(cookies_dir, exist_ok=True)

    try:
        api = Linkedin(email, password, cookies_dir=cookies_dir)
    except Exception as e:
        err_str = str(e)
        if "CHALLENGE" in err_str.upper() or "checkpoint" in err_str.lower():
            print(json.dumps({"error": "LinkedIn demande une vérification (2FA/captcha). Connectez-vous d'abord dans un navigateur."}))
        else:
            print(json.dumps({"error": f"Login LinkedIn échoué: {err_str}"}))
        sys.exit(1)

    try:
        if action == "search":
            results = do_search(api, params)
            print(json.dumps({"ok": True, "results": results}))

        elif action == "send":
            result = do_send(api, params)
            print(json.dumps(result))

        elif action == "profile":
            result = do_profile(api, params)
            print(json.dumps({"ok": True, "profile": result}))

        elif action == "login":
            # Just test login
            print(json.dumps({"ok": True, "message": "Login OK"}))

        elif action == "get_cookie":
            # Extract li_at cookie from session
            cookies = api.client.session.cookies
            li_at = cookies.get("li_at", domain=".linkedin.com")
            if not li_at:
                li_at = cookies.get("li_at")
            if li_at:
                print(json.dumps({"ok": True, "li_at": li_at}))
            else:
                all_cookies = {c.name: c.value[:20] + "..." for c in cookies}
                print(json.dumps({"error": f"li_at non trouvé. Cookies: {all_cookies}"}))

        else:
            print(json.dumps({"error": f"Unknown action: {action}"}))

    except Exception as e:
        print(json.dumps({"error": f"{action}: {str(e)}"}))
        sys.exit(1)


def do_search(api, params):
    keywords = params.get("keywords", "")
    keyword_title = params.get("title")
    limit = params.get("limit", 25)
    offset = params.get("offset", 0)

    results = api.search_people(
        keywords=keywords or None,
        keyword_title=keyword_title or None,
        limit=limit,
    )

    # Apply offset
    results = results[offset:offset + limit]

    contacts = []
    for r in results:
        contacts.append({
            "urn_id": r.get("urn_id", ""),
            "name": r.get("name", ""),
            "jobtitle": r.get("jobtitle", ""),
            "location": r.get("location", ""),
            "public_id": r.get("public_id", ""),
        })

    return contacts


def do_send(api, params):
    recipients = params.get("recipients", [])
    message = params.get("message", "")
    public_id = params.get("public_id")

    if not message:
        return {"error": "message required"}

    # Resolve public_id to urn_id if needed
    if public_id and not recipients:
        try:
            profile = api.get_profile(public_id=public_id)
            urn_id = profile.get("profile_id") or profile.get("member_urn_id")
            if urn_id:
                recipients = [urn_id]
            else:
                available_keys = [k for k in profile.keys() if 'id' in k.lower() or 'urn' in k.lower()]
                return {"error": f"Cannot resolve '{public_id}'. Keys with id/urn: {available_keys}"}
        except Exception as e:
            return {"error": f"Erreur résolution profil '{public_id}': {e}"}

    if not recipients:
        return {"error": "recipients or public_id required"}

    try:
        result = api.send_message(message_body=message, recipients=recipients)
        # send_message returns True if ERROR, False if success
        if result is True:
            return {"error": f"LinkedIn a refusé le message vers {recipients}. Vérifiez que vous êtes connecté avec cette personne."}
        elif result is False or result is None:
            return {"ok": True, "message": "Message envoyé"}
        else:
            # Some versions return the response object
            return {"ok": True, "message": f"Message envoyé (response={result})"}
    except Exception as e:
        return {"error": f"Erreur envoi: {e}"}


def do_profile(api, params):
    public_id = params.get("public_id")
    urn_id = params.get("urn_id")

    profile = api.get_profile(public_id=public_id, urn_id=urn_id)
    return {
        "firstName": profile.get("firstName", ""),
        "lastName": profile.get("lastName", ""),
        "headline": profile.get("headline", ""),
        "publicIdentifier": profile.get("public_id", ""),
        "urn_id": profile.get("profile_id", ""),
        "location": profile.get("locationName", ""),
        "industry": profile.get("industryName", ""),
    }


if __name__ == "__main__":
    main()
