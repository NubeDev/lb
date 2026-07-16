.PHONY: dev serve open clean

PORT ?= 5180

dev serve:
	python3 -m http.server $(PORT)

open:
	xdg-open http://localhost:$(PORT) 2>/dev/null || open http://localhost:$(PORT)

clean:
	find . -name '__pycache__' -exec rm -rf {} +
