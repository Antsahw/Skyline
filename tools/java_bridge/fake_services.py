"""A stand-in for the Minecraft services on localhost, for install_test.sh's owner case: the
account UUID owns Minecraft: Java Edition for the token TOKEN only; every other token is refused.
A java-mods-dev build of the game checks with it (HORIZON_JAVA_MODS_SERVICES); release builds
only ask Mojang's own. Writes the port it listens on to PORT_FILE.

    python3 -I fake_services.py UUID TOKEN PORT_FILE
"""
import http.server
import json
import sys

UUID, TOKEN, PORT_FILE = sys.argv[1:4]
ANSWERS = {
    "/minecraft/profile": {"id": UUID, "name": "Tester"},
    "/entitlements/mcstore": {"items": [{"name": "product_minecraft"}, {"name": "game_minecraft"}]},
}


class Services(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        body = ANSWERS.get(self.path)
        ok = self.headers.get("Authorization") == f"Bearer {TOKEN}" and body is not None
        data = json.dumps(body if ok else {}).encode()
        self.send_response(200 if ok else 401)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


server = http.server.HTTPServer(("127.0.0.1", 0), Services)
with open(PORT_FILE, "w") as f:
    f.write(str(server.server_address[1]))
server.serve_forever()
