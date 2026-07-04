import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/lib/utils";

const badgeVariants = cva(
  "inline-flex items-center rounded-md border px-2 py-0.5 text-xs font-medium transition-colors",
  {
    variants: {
      variant: {
        default: "border-transparent bg-primary text-primary-foreground",
        secondary: "border-border bg-bg text-muted",
        outline: "border-border bg-bg text-muted",
        destructive: "border-destructive/25 bg-destructive/10 text-destructive",
        // Semantic status tones (theme-appearance widened palette). `success`/`warning` are the
        // fixed semantic hues; `accent2` is the secondary accent — all follow the theme by token.
        success: "border-success/25 bg-success/10 text-success",
        warning: "border-warning/30 bg-warning/10 text-warning",
        accent2: "border-accent-2/25 bg-accent-2/10 text-accent-2",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
);

function Badge({
  className,
  variant,
  ...props
}: React.ComponentProps<"span"> & VariantProps<typeof badgeVariants>) {
  return <span data-slot="badge" className={cn(badgeVariants({ variant }), className)} {...props} />;
}

export { Badge, badgeVariants };
