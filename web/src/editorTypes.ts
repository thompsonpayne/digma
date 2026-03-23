export type Point = {
  x: number;
  y: number;
};

export type CameraView = {
  pan: Point;
  zoom: number;
};

export const ToolMode = {
  select: "select",
  rect: "rect",
} as const;

export type ToolMode = (typeof ToolMode)[keyof typeof ToolMode];

export type InputEvent =
  | { type: "camera_pan_by_screen_delta"; delta_px: Point }
  | {
      type: "camera_zoom_at_screen_point";
      pivot_px: Point;
      zoom_multiplier: number;
    }
  | { type: "pointer_down"; screen_px: Point; shift: boolean; button: number }
  | { type: "pointer_up"; screen_px: Point; button: number }
  | { type: "pointer_move"; screen_px: Point; buttons: number }
  | { type: "pointer_cancel" }
  | { type: "set_selection_fill"; color: RgbaColor }
  | { type: "undo" }
  | { type: "redo" };

export type InputBatch = {
  events: InputEvent[];
  tool: ToolMode;
};

export type TickOutput = {
  camera: CameraView;
  cursor: string;
};

export type RgbaColor = {
  r: number;
  g: number;
  b: number;
  a: number;
};
