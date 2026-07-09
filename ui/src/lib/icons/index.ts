// The icon lib: a string-named, searchable wrapper over lucide-react so features and
// extensions can *store* an icon as opaque data ("git-branch") and render/pick it later.
//
//   import { Icon, IconPicker, resolveIcon } from "@/lib/icons";
//   <Icon name={cell.icon} fallback="box" className="size-4" />
//   <IconPicker value={icon} onSelect={setIcon} pageSize={10} />
//
// Barrel only — implementations live one-per-file (FILE-LAYOUT).

export { Icon, type IconProps } from "./Icon";
export { IconPicker, type IconPickerProps } from "./IconPicker";
export { resolveIcon, isIconName } from "./resolve";
export { ICON_CATALOG, searchIcons, type IconEntry } from "./catalog";
