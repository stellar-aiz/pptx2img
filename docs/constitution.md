# Software Design Charter

pptx2img converts PPTX slides into images according to four design principles:
lightweightness, fidelity, transparency, and observability.

These principles guide the software's behavior and architecture. Development
procedures are documented separately in [AGENTS.md](../AGENTS.md) and the
[development guide](development.md).

## Lightweightness

Keep resource consumption and runtime dependencies small. Conversion must run
standalone, without requiring Office, LibreOffice, a browser, or an external
service. Third-party libraries are compatible with this principle; their runtime
cost and deployment requirements matter.

## Fidelity

Reproduce the original slide's content, appearance, and layout as faithfully as
possible. Preserve text, shapes, and their placement, and respect the formatting
expressed in the presentation.

## Transparency

Make substitutions, approximations, omissions, and unsupported features visible
to the user. Report limitations alongside the output so users can judge whether
the result is suitable for their purpose.

## Observability

Make rendering decisions and their resulting output traceable. Expose enough
diagnostic information for people and agents to identify the causes of
differences, make targeted corrections, and verify their effects. Observability
enables autonomous improvement; it does not imply that the renderer modifies
itself.
