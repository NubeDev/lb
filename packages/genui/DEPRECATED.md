# `@nube/genui` is deprecated

**The shared substrate moved to [`NubeDev/lb-ui-kit`](https://github.com/NubeDev/lb-ui-kit)
(`@nube/dash-kit`)** — see `../README.md`.

## Status: kept, not deleted — deliberately

`genui` (the agent-authors-a-widget layer) is the kit's **Tier 2**, and Tier 2 is explicitly *not
started*: it lands only once a real consumer defines its edge
(rubix-ai `docs/scope/ui/ext-ui-kit-scope.md` §4). Until the kit absorbs it, the live copy is
rubix-ai's vendored `ui/packages/genui`, and this directory is a signpost rather than a home.

Deleting it now would remove the artifact the Tier 2 decision is about, while adding a consumer here
would make lb a second substrate again — which is the whole failure this deprecation exists to close.

## Do not add a consumer

New work takes the kit. If you need generative-UI rendering in an extension, that is the Tier 2 ask on
[rubix-ai#152](https://github.com/NubeIO/rubix-ai/issues/152).
