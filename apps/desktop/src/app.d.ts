import type { CellData, RowData, TableFeatures } from "@tanstack/table-core";

declare module "@tanstack/table-core" {
  interface ColumnMeta<
    in out TFeatures extends TableFeatures,
    in out TData extends RowData,
    TValue extends CellData = CellData,
  > {
    headerClass?: string;
    cellClass?: string;
  }
}
