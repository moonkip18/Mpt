import 'package:flutter/material.dart';
import 'package:window_manager/window_manager.dart';

import '../core/theme/mpt_theme.dart';

/// Frameless-window chrome: drag area plus the only three window buttons.
class MptTitleBar extends StatelessWidget {
  const MptTitleBar({
    super.key,
    this.trailing,
    this.onToggleMini,
    this.isMini = false,
  });

  final Widget? trailing;
  final VoidCallback? onToggleMini;
  final bool isMini;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 38,
      child: Row(
        children: [
          Expanded(
            child: DragToMoveArea(
              child: Padding(
                padding: const EdgeInsets.only(left: 16),
                child: Row(
                  children: [
                    ClipRRect(
                      borderRadius: BorderRadius.circular(3),
                      child: Image.asset(
                        'assets/images/logo.png',
                        width: 17,
                        height: 17,
                        filterQuality: FilterQuality.medium,
                      ),
                    ),
                    const SizedBox(width: 10),
                    const Text(
                      'MPT',
                      style: TextStyle(
                        fontFamily: MptFonts.ui,
                        fontSize: 11,
                        letterSpacing: 3,
                        fontWeight: FontWeight.w600,
                        color: MptColors.inkDim,
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
          if (trailing != null) ...[
            trailing!,
            const SizedBox(width: 4),
          ],
          if (onToggleMini != null)
            _ChromeButton(
              icon: isMini
                  ? Icons.open_in_full_rounded
                  : Icons.picture_in_picture_alt_outlined,
              tooltip: isMini ? 'Back to full size' : 'Mini player',
              onTap: onToggleMini!,
            ),
          _ChromeButton(
            icon: Icons.remove_rounded,
            tooltip: 'Minimize',
            onTap: () => windowManager.minimize(),
          ),
          _ChromeButton(
            icon: Icons.crop_square_rounded,
            tooltip: 'Maximize',
            onTap: () async {
              if (await windowManager.isMaximized()) {
                await windowManager.unmaximize();
              } else {
                await windowManager.maximize();
              }
            },
          ),
          _ChromeButton(
            icon: Icons.close_rounded,
            tooltip: 'Close',
            isDanger: true,
            onTap: () => windowManager.close(),
          ),
          const SizedBox(width: 4),
        ],
      ),
    );
  }
}

class _ChromeButton extends StatelessWidget {
  const _ChromeButton({
    required this.icon,
    required this.tooltip,
    required this.onTap,
    this.isDanger = false,
  });

  final IconData icon;
  final String tooltip;
  final VoidCallback onTap;
  final bool isDanger;

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tooltip,
      child: InkWell(
        onTap: onTap,
        hoverColor: isDanger ? MptColors.accentSoft : MptColors.line,
        child: SizedBox(
          width: 40,
          height: 38,
          child: Icon(
            icon,
            size: 15,
            color: isDanger ? MptColors.inkDim : MptColors.inkDim,
          ),
        ),
      ),
    );
  }
}
