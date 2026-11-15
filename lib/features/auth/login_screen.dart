import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/config/env_config.dart';
import '../../core/theme/mpt_theme.dart';
import '../../shared/title_bar.dart';
import 'auth_notifier.dart';

class LoginScreen extends StatelessWidget {
  const LoginScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: MptColors.background,
      body: Column(
        children: [
          // The window is frameless, so the login screen needs the same drag
          // area and close button as the study screen.
          const MptTitleBar(),
          Container(height: 1, color: MptColors.line),
          Expanded(
            child: Stack(
              children: [
                const Positioned.fill(child: _GroundGradient()),
                Center(
                  child: SingleChildScrollView(
                    padding: const EdgeInsets.symmetric(
                        vertical: 40, horizontal: 24),
                    child: ConstrainedBox(
                      constraints: const BoxConstraints(maxWidth: 360),
                      child: const _LoginForm(),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _LoginForm extends ConsumerStatefulWidget {
  const _LoginForm();

  @override
  ConsumerState<_LoginForm> createState() => _LoginFormState();
}

class _LoginFormState extends ConsumerState<_LoginForm> {
  final _email = TextEditingController();
  final _password = TextEditingController();
  final _passwordFocus = FocusNode();
  bool _obscure = true;
  String? _localError;

  @override
  void dispose() {
    _email.dispose();
    _password.dispose();
    _passwordFocus.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    final email = _email.text.trim();
    final password = _password.text;
    if (email.isEmpty || !email.contains('@')) {
      setState(() => _localError = 'Enter the e-mail used in Yeolpumta.');
      return;
    }
    if (password.isEmpty) {
      setState(() => _localError = 'Enter your password.');
      return;
    }
    setState(() => _localError = null);
    await ref.read(authStateProvider.notifier).signInWithEmail(
          email: email,
          password: password,
        );
  }

  @override
  Widget build(BuildContext context) {
    final auth = ref.watch(authStateProvider);
    final busy = auth.isLoading;
    final error = _localError ?? auth.errorMessage;
    final canUseGoogle = EnvConfig.googleClientId.isNotEmpty;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Row(
          children: [
            Container(
              width: 7,
              height: 7,
              decoration: const BoxDecoration(
                color: MptColors.accent,
                shape: BoxShape.circle,
              ),
            ),
            const SizedBox(width: 10),
            const Text('MPT', style: _brand),
          ],
        ),
        const SizedBox(height: 28),
        const Text('Sign in to YPT', style: _headline),
        const SizedBox(height: 10),
        const Text(
          'Your timer and subjects stay in sync with the phone app. '
          'Time studied here is written to the same account.',
          style: _sub,
        ),
        const SizedBox(height: 32),
        _Field(
          controller: _email,
          hint: 'E-mail',
          keyboardType: TextInputType.emailAddress,
          textInputAction: TextInputAction.next,
          onSubmitted: (_) => _passwordFocus.requestFocus(),
        ),
        const SizedBox(height: 12),
        _Field(
          controller: _password,
          hint: 'Password',
          focusNode: _passwordFocus,
          obscure: _obscure,
          textInputAction: TextInputAction.done,
          onSubmitted: (_) => _submit(),
          suffix: IconButton(
            splashRadius: 16,
            icon: Icon(
              _obscure
                  ? Icons.visibility_off_outlined
                  : Icons.visibility_outlined,
              size: 15,
              color: MptColors.inkFaint,
            ),
            onPressed: () => setState(() => _obscure = !_obscure),
          ),
        ),
        if (error != null) ...[
          const SizedBox(height: 14),
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Container(
                margin: const EdgeInsets.only(top: 5),
                width: 4,
                height: 4,
                decoration: const BoxDecoration(
                  color: MptColors.accent,
                  shape: BoxShape.circle,
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  error,
                  style: const TextStyle(
                    fontFamily: MptFonts.ui,
                    fontSize: 12,
                    height: 1.4,
                    color: MptColors.accent,
                  ),
                ),
              ),
            ],
          ),
        ],
        const SizedBox(height: 24),
        _PrimaryButton(
          label: busy ? 'Signing in…' : 'Sign in',
          busy: busy,
          onTap: busy ? null : _submit,
        ),
        if (canUseGoogle) ...[
          const SizedBox(height: 10),
          _GhostButton(
            label: 'Continue with Google',
            onTap: busy
                ? null
                : () => ref.read(authStateProvider.notifier).signInWithGoogle(),
          ),
        ],
        const SizedBox(height: 26),
        const Text(
          'This is an unofficial desktop client. Use the same account '
          'you already have on Yeolpumta.',
          style: TextStyle(
            fontFamily: MptFonts.ui,
            fontSize: 11,
            height: 1.6,
            color: MptColors.inkFaint,
          ),
        ),
      ],
    );
  }
}

class _GroundGradient extends StatelessWidget {
  const _GroundGradient();

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        gradient: LinearGradient(
          begin: Alignment.topCenter,
          end: Alignment.bottomCenter,
          colors: [
            MptColors.surface.withValues(alpha: 0.85),
            MptColors.background,
          ],
          stops: const [0, 0.55],
        ),
      ),
    );
  }
}

class _Field extends StatelessWidget {
  const _Field({
    required this.controller,
    required this.hint,
    this.keyboardType,
    this.textInputAction,
    this.onSubmitted,
    this.focusNode,
    this.obscure = false,
    this.suffix,
  });

  final TextEditingController controller;
  final String hint;
  final TextInputType? keyboardType;
  final TextInputAction? textInputAction;
  final ValueChanged<String>? onSubmitted;
  final FocusNode? focusNode;
  final bool obscure;
  final Widget? suffix;

  @override
  Widget build(BuildContext context) {
    return TextField(
      controller: controller,
      focusNode: focusNode,
      obscureText: obscure,
      keyboardType: keyboardType,
      textInputAction: textInputAction,
      onSubmitted: onSubmitted,
      cursorColor: MptColors.ink,
      cursorWidth: 1,
      style: const TextStyle(
        fontFamily: MptFonts.ui,
        fontSize: 14,
        color: MptColors.ink,
      ),
      decoration: InputDecoration(
        hintText: hint,
        hintStyle: const TextStyle(
          fontFamily: MptFonts.ui,
          fontSize: 14,
          color: MptColors.inkFaint,
        ),
        filled: true,
        fillColor: MptColors.surface,
        suffixIcon: suffix,
        contentPadding:
            const EdgeInsets.symmetric(horizontal: 14, vertical: 15),
        enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(3),
          borderSide: const BorderSide(color: MptColors.line),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(3),
          borderSide: const BorderSide(color: MptColors.inkFaint),
        ),
      ),
    );
  }
}

class _PrimaryButton extends StatelessWidget {
  const _PrimaryButton(
      {required this.label, required this.onTap, this.busy = false});

  final String label;
  final VoidCallback? onTap;
  final bool busy;

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      child: Container(
        height: 46,
        alignment: Alignment.center,
        decoration: BoxDecoration(
          color: MptColors.synapse,
          borderRadius: BorderRadius.circular(3),
        ),
        child: busy
            ? const SizedBox(
                width: 14,
                height: 14,
                child: CircularProgressIndicator(
                    strokeWidth: 1.4, color: MptColors.background),
              )
            : Text(
                label,
                style: const TextStyle(
                  fontFamily: MptFonts.ui,
                  fontSize: 13,
                  fontWeight: FontWeight.w600,
                  letterSpacing: 0.2,
                  color: MptColors.background,
                ),
              ),
      ),
    );
  }
}

class _GhostButton extends StatelessWidget {
  const _GhostButton({required this.label, required this.onTap});

  final String label;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      child: Container(
        height: 46,
        alignment: Alignment.center,
        decoration: BoxDecoration(
          border: Border.all(color: MptColors.lineStrong),
          borderRadius: BorderRadius.circular(3),
        ),
        child: Text(
          label,
          style: const TextStyle(
            fontFamily: MptFonts.ui,
            fontSize: 13,
            fontWeight: FontWeight.w500,
            color: MptColors.ink,
          ),
        ),
      ),
    );
  }
}

const _brand = TextStyle(
  fontFamily: MptFonts.ui,
  fontSize: 11,
  letterSpacing: 3,
  fontWeight: FontWeight.w600,
  color: MptColors.inkDim,
);

const _headline = TextStyle(
  fontFamily: MptFonts.display,
  fontSize: 38,
  height: 1.05,
  fontWeight: FontWeight.w400,
  letterSpacing: -0.8,
  color: MptColors.ink,
);

const _sub = TextStyle(
  fontFamily: MptFonts.ui,
  fontSize: 13,
  height: 1.6,
  color: MptColors.inkDim,
);
