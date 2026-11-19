"""Minimal HTTP client for the NEXUS FABRIC API (standard library only)."""
import json
import os
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
from http.cookiejar import CookieJar

USER, PASSWORD = 'validator', 'independent-check-7'


class Client:
    def __init__(self, base):
        self.base = base.rstrip('/')
        self.opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(CookieJar()), urllib.request.ProxyHandler({}))
        self.project = None
        self._cred = None
        self.engine = None

    def raw(self, method, path, body=None, raw_body=None):
        """Returns (status, bytes).  `raw_body` sends text exactly as given (for malformed JSON tests)."""
        data = raw_body.encode() if raw_body is not None else (json.dumps(body).encode() if body is not None else None)
        req = urllib.request.Request(self.base + path, data=data, method=method, headers={'X-NF-Request': '1', 'Content-Type': 'application/json'})
        for attempt in range(40):
            try:
                with self.opener.open(req, timeout=120) as r:
                    return r.status, r.read()
            except urllib.error.HTTPError as e:
                if e.code == 401 and self._cred and not path.startswith('/api/login') and attempt < 2:
                    self.login(*self._cred)      # session lost (server restarted): sign in again and retry
                    continue
                return e.code, e.read()
            except (urllib.error.URLError, ConnectionError, TimeoutError):
                time.sleep(1.5)                  # server restarting: wait, never report this as a product result
        raise RuntimeError('server unreachable for 60 s: ' + path)

    def json(self, method, path, body=None):
        st, b = self.raw(method, path, body)
        try:
            return st, json.loads(b)
        except Exception:
            return st, {'_raw': b.decode('utf-8', 'replace')}

    def connect(self, user=USER, password=PASSWORD):
        st, h = self.json('GET', '/api/health')
        if st != 200:
            raise RuntimeError(f'server not healthy: {st}')
        if h.get('setup_required'):
            self.json('POST', '/api/setup', {'username': user, 'password': password})
        self.login(user, password)
        st, r = self.json('POST', '/api/projects', {'name': 'computational validation ' + time.strftime('%Y%m%d-%H%M%S'), 'description': 'independent validation run'})
        if st != 200:
            raise RuntimeError(f'cannot create project: {st} {r}')
        self.project = r['id']
        self.engine = h.get('engine')
        return self

    def login(self, user=USER, password=PASSWORD):
        st, r = self.json('POST', '/api/login', {'username': user, 'password': password})
        if st != 200:
            raise RuntimeError(f'login failed ({st}): {r}. Give credentials with --user/--password.')
        self._cred = (user, password)

    def calculate(self, request=None, raw_body=None):
        """Submit a request.  Returns dict: http, id, result (job JSON or None), error."""
        st, b = self.raw('POST', f'/api/projects/{self.project}/calculations', request, raw_body)
        out = {'http': st, 'id': None, 'result': None, 'error': None}
        try:
            r = json.loads(b)
        except Exception:
            out['error'] = b.decode('utf-8', 'replace')
            return out
        if st != 200:
            out['error'] = r.get('error', str(r))
            return out
        out['id'] = r['id']
        st2, res = self.json('GET', f"/api/calculations/{r['id']}")
        out['result'] = res if st2 == 200 else None
        return out

    def dxf(self, calc_id, part):
        st, b = self.raw('GET', f'/api/calculations/{calc_id}/dxf/{part}')
        return st, b.decode('utf-8', 'replace')


class SpawnedServer:
    """Starts a private copy of nf-server on a temporary data directory and restarts it if it is killed."""

    def __init__(self, binary, port=8802):
        import threading
        self.binary, self.port = binary, port
        self.dir = tempfile.mkdtemp(prefix='nf-validation-')
        self.url = f'http://127.0.0.1:{port}'
        self.log = open(os.path.join(self.dir, 'server.log'), 'a')
        self.restarts, self._stop = 0, False
        self._start()
        threading.Thread(target=self._watch, daemon=True).start()

    def _start(self):
        env = dict(os.environ, NF_DATA_DIR=self.dir, NF_LISTEN=f'127.0.0.1:{self.port}')
        self.proc = subprocess.Popen([self.binary], env=env, stdout=self.log, stderr=subprocess.STDOUT)
        for _ in range(100):
            try:
                urllib.request.build_opener(urllib.request.ProxyHandler({})).open(self.url + '/api/health', timeout=1).read()
                return
            except Exception:
                time.sleep(0.1)
        raise RuntimeError('server did not start')

    def _watch(self):
        while not self._stop:
            time.sleep(1.0)
            if self.proc.poll() is not None and not self._stop:
                self.restarts += 1
                try:
                    self._start()
                except Exception:
                    pass

    def stop(self):
        self._stop = True
        self.proc.terminate()
        self.log.close()
