// The add-datasource dialog (rules-workbench scope) — wraps `AddDatasourceForm` in the shadcn
// `Dialog` primitive so creating a new datasource is a focused pop-out (focus trapping +
// Escape/overlay dismissal for free), matching the action-in-header pattern every other surface
// uses (the Webhooks "New webhook" button, the Dashboards "Delete" action). The trigger lives in
// the page header's `actions` slot; the form itself is unchanged. Closes on submit. One
// responsibility, one file (FILE-LAYOUT).

import { useState } from "react";
import { Database } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { AddDatasource } from "@/lib/datasources";
import { AddDatasourceForm } from "./AddDatasourceForm";

interface Props {
  onAdd: (input: AddDatasource) => void;
}

export function AddDatasourceDialog({ onAdd }: Props) {
  const [open, setOpen] = useState(false);
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <Button aria-label="new datasource" size="sm" onClick={() => setOpen(true)}>
        <Database size={13} /> New datasource
      </Button>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Add a datasource</DialogTitle>
          <DialogDescription>
            Register an external source the workspace can query. The DSN is held only until submit.
          </DialogDescription>
        </DialogHeader>
        <AddDatasourceForm
          onAdd={(input) => {
            onAdd(input);
            setOpen(false);
          }}
        />
      </DialogContent>
    </Dialog>
  );
}
