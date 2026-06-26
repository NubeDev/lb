// View/DTO types for the assets surface — mirror the Rust `lb_assets` models one-to-one
// (FILE-LAYOUT: the type has the same name across the Rust model, the DTO, and the client).

/** A document asset summary, as the node speaks it (the `get_doc`/`list_docs` shape). */
export interface Doc {
  id: string;
  title: string;
  /** Present on a full `get_doc`; omitted in a `list_docs` summary. */
  content?: string;
  owner?: string;
}

/** A loaded skill (the `load_skill` shape). */
export interface Skill {
  id: string;
  version: string;
  body: string;
}
