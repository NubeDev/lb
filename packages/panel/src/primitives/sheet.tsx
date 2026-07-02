// Vendored from shadcn/ui (via lb's components/ui/sheet.tsx and @nube/nav-rail). Backs the
// Panel's right-docked overlay. Colors → --lbp-* tokens. Unlike the stock sheet, the
// docked content takes NO fixed `sm:max-w-*` — the Panel drives its width via a controlled
// style so the resize handle can widen it. Radix gives us the overlay, focus trap, escape,
// and portal for free (same primitive nav-rail vendored).

import * as React from "react";
import * as SheetPrimitive from "@radix-ui/react-dialog";

import { cn } from "../lib/cn";

function Sheet({ ...props }: React.ComponentProps<typeof SheetPrimitive.Root>) {
  return <SheetPrimitive.Root {...props} />;
}

function SheetPortal({ ...props }: React.ComponentProps<typeof SheetPrimitive.Portal>) {
  return <SheetPrimitive.Portal {...props} />;
}

const SheetOverlay = React.forwardRef<
  React.ElementRef<typeof SheetPrimitive.Overlay>,
  React.ComponentPropsWithoutRef<typeof SheetPrimitive.Overlay>
>(function SheetOverlay({ className, ...props }, ref) {
  return (
    <SheetPrimitive.Overlay
      ref={ref}
      className={cn("fixed inset-0 z-50 bg-black/50", className)}
      {...props}
    />
  );
});

/**
 * The right-docked, WIDTH-CONTROLLED surface. Width comes from the caller (the Panel's
 * resize state) via `style.width` — not a Tailwind max-width — so dragging widens it.
 * `.lb-panel` is applied here so the portal root carries the panel's scoped tokens.
 */
const SheetContent = React.forwardRef<
  React.ElementRef<typeof SheetPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof SheetPrimitive.Content>
>(function SheetContent({ className, children, ...props }, ref) {
  return (
    <SheetPortal>
      <SheetOverlay />
      <SheetPrimitive.Content
        ref={ref}
        className={cn(
          "lb-panel fixed inset-y-0 right-0 z-50 flex h-full max-w-[95vw] flex-col border-l border-lbp-border bg-lbp-panel font-sans text-lbp-fg shadow-2xl outline-none",
          className,
        )}
        {...props}
      >
        {children}
      </SheetPrimitive.Content>
    </SheetPortal>
  );
});

const SheetTitle = React.forwardRef<
  React.ElementRef<typeof SheetPrimitive.Title>,
  React.ComponentPropsWithoutRef<typeof SheetPrimitive.Title>
>(function SheetTitle({ className, ...props }, ref) {
  return <SheetPrimitive.Title ref={ref} className={cn("text-base font-semibold text-lbp-fg", className)} {...props} />;
});

const SheetDescription = React.forwardRef<
  React.ElementRef<typeof SheetPrimitive.Description>,
  React.ComponentPropsWithoutRef<typeof SheetPrimitive.Description>
>(function SheetDescription({ className, ...props }, ref) {
  return (
    <SheetPrimitive.Description ref={ref} className={cn("text-xs text-lbp-muted", className)} {...props} />
  );
});

export { Sheet, SheetContent, SheetTitle, SheetDescription };
