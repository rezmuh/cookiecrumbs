"""
WSGI config for cookiecrumbs project.
"""

import os

from django.core.wsgi import get_wsgi_application

os.environ.setdefault("DJANGO_SETTINGS_MODULE", "cookiecrumbs.settings")

application = get_wsgi_application()
