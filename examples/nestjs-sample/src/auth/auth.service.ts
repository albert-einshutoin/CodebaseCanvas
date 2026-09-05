import { Injectable } from '@nestjs/common';
import { TokenRepository } from './token.repository';

@Injectable()
export class AuthService {
  constructor(private readonly tokens: TokenRepository) {}
  login() {
    this.normalize();
    return this.tokens.save();
  }
  normalize() { return 'normalized'; }
}
