# Visibility

Retained content whose visibility can change without reflow.

## When to use

Use it for staged content or any UI where hiding a region should reserve its space. Use collapse when hidden content should take no room.

## What you provide

Supply visible and children. Invisible content retains layout and identity but has no paint, pointer input, accessibility presence or Tab stops. This component does not cancel timers; the presentation controller manages its own activation work.

The generated [component reference](../../../bindings/reference.md) lists every prop and binding spelling.

## Do

- Reuse project fonts, colors and sizes through Settings.
- Check the composition at narrow widths and in both appearances.

## Don't

- Bake a specific presentation theme into application content.
- Rely on invisible content to keep background work running or stop it.
